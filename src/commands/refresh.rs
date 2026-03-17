use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

use rusqlite::Connection;

use crate::config;
use crate::config::repo_config;
use crate::core::discovery_service::{self, RepoSnapshot};
use crate::core::provider_service;
use crate::db::{env_repo, repo_repo, topic_env_repo, topic_repo};
use crate::error::Result;
use crate::id::{RepoId, TopicId};

struct SemaphoreGuard<'a> {
    sem: &'a (Mutex<usize>, Condvar),
}

impl<'a> SemaphoreGuard<'a> {
    fn acquire(sem: &'a (Mutex<usize>, Condvar)) -> Self {
        let (lock, cvar) = sem;
        let mut count = lock.lock().unwrap();
        while *count == 0 {
            count = cvar.wait(count).unwrap();
        }
        *count -= 1;
        Self { sem }
    }
}

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        let (lock, cvar) = self.sem;
        let mut count = lock.lock().unwrap();
        *count += 1;
        cvar.notify_one();
    }
}

pub fn handle_refresh(
    conn: &Connection,
    repo_id: Option<&str>,
    workspace_root: &Path,
) -> Result<String> {
    let config_path = workspace_root.join(".restack/config.toml");
    let cfg = Arc::new(if config_path.exists() {
        config::load_config(&config_path)?
    } else {
        config::default_config()
    });

    let repos = if let Some(id) = repo_id {
        let rid = id
            .parse()
            .map_err(|_| crate::error::RestackError::RepoNotFoundByName(id.to_string()))?;
        vec![repo_repo::get_repo(conn, &rid)?]
    } else {
        repo_repo::list_repos(conn)?
    };

    // Pre-read all snapshot data in one pass (avoids N×M DB queries later)
    let all_topics = topic_repo::list_topics(conn, None)?;
    let all_envs = env_repo::list_envs(conn, None)?;
    let all_topic_envs = topic_env_repo::list_all_topic_environments(conn)?;

    // Build topic_id → repo_id index so we can route topic_envs to the right snapshot
    let topic_to_repo: HashMap<TopicId, RepoId> = all_topics
        .iter()
        .map(|t| (t.id.clone(), t.repo_id.clone()))
        .collect();

    // Group into per-repo snapshots
    let mut snapshots: HashMap<RepoId, RepoSnapshot> = HashMap::new();
    for topic in all_topics {
        snapshots
            .entry(topic.repo_id.clone())
            .or_insert_with(|| RepoSnapshot {
                topics: Vec::new(),
                envs: Vec::new(),
                topic_envs: HashMap::new(),
            })
            .topics
            .push(topic);
    }
    for env in all_envs {
        snapshots
            .entry(env.repo_id.clone())
            .or_insert_with(|| RepoSnapshot {
                topics: Vec::new(),
                envs: Vec::new(),
                topic_envs: HashMap::new(),
            })
            .envs
            .push(env);
    }
    for te in all_topic_envs {
        if let Some(repo_id) = topic_to_repo.get(&te.topic_id) {
            if let Some(snapshot) = snapshots.get_mut(repo_id) {
                snapshot
                    .topic_envs
                    .entry(te.topic_id)
                    .or_default()
                    .push(te.env_id);
            }
        }
    }

    // Pair each repo with its snapshot for thread dispatch
    let repo_snapshots: Vec<_> = repos
        .into_iter()
        .map(|repo| {
            let snapshot = snapshots.remove(&repo.id).unwrap_or_else(|| RepoSnapshot {
                topics: Vec::new(),
                envs: Vec::new(),
                topic_envs: HashMap::new(),
            });
            (repo, snapshot)
        })
        .collect();

    // Bounded concurrency semaphore (global limit: 8)
    const MAX_CONCURRENT: usize = 8;
    let semaphore = Arc::new((Mutex::new(MAX_CONCURRENT), Condvar::new()));

    // Phase 1+2: parallel git fetch + discovery (no DB access)
    let plan_results: Vec<_> = std::thread::scope(|s| {
        let handles: Vec<_> = repo_snapshots
            .iter()
            .map(|(repo, snapshot)| {
                let sem = Arc::clone(&semaphore);
                let cfg = Arc::clone(&cfg);
                s.spawn(move || {
                    let _guard = SemaphoreGuard::acquire(&sem);
                    let repo_path = Path::new(&repo.path);
                    let repo_config =
                        repo_config::load_repo_config(&repo_path.join(".restack.yml")).ok();
                    let result = discovery_service::discover_topics_plan(
                        repo,
                        snapshot,
                        &cfg,
                        repo_config.as_ref(),
                    );
                    (repo, result)
                })
            })
            .collect();

        handles
            .into_iter()
            .filter_map(|h| match h.join() {
                Ok(result) => Some(result),
                Err(payload) => {
                    let msg = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                        .unwrap_or("unknown panic");
                    eprintln!("Warning: refresh thread panicked: {msg}");
                    None
                }
            })
            .collect()
    });

    // Phase 3: serial apply (DB writes, single connection)
    let mut json_results = Vec::new();
    for (repo, result) in plan_results {
        match result {
            Ok((mutations, mut discovery, fingerprint, merge_data)) => {
                if !mutations.is_empty() || merge_data.is_some() {
                    discovery_service::apply_mutations(
                        conn,
                        &repo.id,
                        &mutations,
                        merge_data.as_ref(),
                    )?;
                }
                if let Some(fp) = fingerprint {
                    let now = chrono::Utc::now().to_rfc3339();
                    repo_repo::update_repo_fingerprint(conn, &repo.id, &fp, &now)?;
                }
                discovery.topics = topic_repo::list_topics(conn, Some(&repo.id))?;

                let ci_refresh = if cfg.provider.auto_ci_refresh {
                    provider_service::refresh_ci_statuses(conn, repo)?
                } else {
                    Vec::new()
                };

                json_results.push(serde_json::json!({
                    "repo": repo.name,
                    "discovery": discovery,
                    "ci_refreshed": ci_refresh.len(),
                }));
            }
            Err(e) => {
                json_results.push(serde_json::json!({
                    "repo": repo.name,
                    "error": e.to_string(),
                }));
            }
        }
    }

    Ok(serde_json::to_string_pretty(&json_results)?)
}
