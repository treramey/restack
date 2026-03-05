use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::{provider_service, rebuild_service};
use crate::db::{env_repo, repo_repo, topic_env_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::id::RepoId;
use crate::types::{CiStatus, Environment, Rebuild, Topic, TopicStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromoteResult {
    pub topic: Topic,
    pub env: Environment,
    pub rebuild: Option<Rebuild>,
    pub dry_run: bool,
}

pub fn promote_to(
    conn: &Connection,
    topic_id_or_branch: &str,
    env_name: &str,
    repo_id: &RepoId,
    _repo_path: &Path,
    dry_run: bool,
) -> Result<PromoteResult> {
    let topic = resolve_topic(conn, topic_id_or_branch, repo_id)?;
    let env = resolve_env(conn, env_name, repo_id)?;

    // Use the path registered for this specific repo, not the caller-supplied repo_path
    let repo = repo_repo::get_repo(conn, repo_id)?;
    let this_repo_path = std::path::Path::new(&repo.path);

    if !dry_run {
        // Add topic to environment
        topic_env_repo::add_topic_to_env(conn, &topic.id, &env.id)?;

        // Trigger rebuild
        let rebuild = rebuild_service::rebuild_env(conn, &env.id, this_repo_path, false, false)?;
        Ok(PromoteResult {
            topic,
            env,
            rebuild: Some(rebuild),
            dry_run,
        })
    } else {
        Ok(PromoteResult {
            topic,
            env,
            rebuild: None,
            dry_run,
        })
    }
}

pub fn demote_from(
    conn: &Connection,
    topic_id_or_branch: &str,
    env_name: &str,
    repo_id: &RepoId,
    _repo_path: &Path,
    dry_run: bool,
) -> Result<PromoteResult> {
    let topic = resolve_topic(conn, topic_id_or_branch, repo_id)?;
    let env = resolve_env(conn, env_name, repo_id)?;

    // Use the path registered for this specific repo, not the caller-supplied repo_path
    let repo = repo_repo::get_repo(conn, repo_id)?;
    let this_repo_path = std::path::Path::new(&repo.path);

    if !dry_run {
        // Remove topic from environment
        topic_env_repo::remove_topic_from_env(conn, &topic.id, &env.id)?;

        // Trigger rebuild
        let rebuild = rebuild_service::rebuild_env(conn, &env.id, this_repo_path, false, false)?;
        Ok(PromoteResult {
            topic,
            env,
            rebuild: Some(rebuild),
            dry_run,
        })
    } else {
        Ok(PromoteResult {
            topic,
            env,
            rebuild: None,
            dry_run,
        })
    }
}

fn resolve_topic(conn: &Connection, id_or_branch: &str, repo_id: &RepoId) -> Result<Topic> {
    if let Ok(id) = id_or_branch.parse() {
        if let Ok(topic) = topic_repo::get_topic(conn, &id) {
            return Ok(topic);
        }
    }
    match topic_repo::get_topic_by_branch(conn, repo_id, id_or_branch)? {
        Some(topic) => Ok(topic),
        None => Err(RestackError::TopicNotFound(
            id_or_branch.parse().unwrap_or_default(),
        )),
    }
}

fn resolve_env(conn: &Connection, name: &str, repo_id: &RepoId) -> Result<Environment> {
    match env_repo::get_env_by_name(conn, repo_id, name)? {
        Some(env) => Ok(env),
        None => Err(RestackError::EnvNotFound(name.parse().unwrap_or_default())),
    }
}

// ---------------------------------------------------------------------------
// Auto-promote: promote CI-passed topics to auto_promote environments
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoPromoteResult {
    pub refreshed_topics: usize,
    pub promoted: Vec<PromoteResult>,
    pub envs_changed: Vec<String>,
}

/// Refresh CI statuses for all repos, then promote any topic with
/// `ci_status == Passed` into every `auto_promote` environment it is not
/// already in.
pub fn promote_auto(conn: &Connection) -> Result<AutoPromoteResult> {
    let repos = repo_repo::list_repos(conn)?;
    let mut promoted = Vec::new();
    let mut envs_changed = std::collections::HashSet::new();
    let mut refreshed_count = 0usize;

    for repo in &repos {
        // Use the path registered for this specific repo, not the caller-supplied repo_path
        let this_repo_path = std::path::Path::new(&repo.path);

        // Refresh CI statuses via provider
        let entries = provider_service::refresh_ci_statuses(conn, repo).unwrap_or_default();
        refreshed_count += entries.len();

        // Find auto_promote environments for this repo
        let envs = env_repo::list_envs(conn, Some(&repo.id))?;
        let auto_envs: Vec<&Environment> = envs.iter().filter(|e| e.auto_promote).collect();

        if auto_envs.is_empty() {
            continue;
        }

        // Find topics with CI passed + active status
        let topics = topic_repo::list_topics(conn, Some(&repo.id))?;
        let passed_topics: Vec<&Topic> = topics
            .iter()
            .filter(|t| t.status == TopicStatus::Active && t.ci_status == Some(CiStatus::Passed))
            .collect();

        // Collect envs that need a rebuild after all promotions are recorded
        let mut envs_needing_rebuild: std::collections::HashSet<crate::id::EnvId> =
            std::collections::HashSet::new();

        for topic in passed_topics {
            let current_envs = topic_env_repo::get_envs_for_topic(conn, &topic.id)?;

            for env in &auto_envs {
                if current_envs.contains(&env.id) {
                    continue;
                }

                // Promote this topic into the auto_promote env
                topic_env_repo::add_topic_to_env(conn, &topic.id, &env.id)?;
                envs_needing_rebuild.insert(env.id.clone());
                envs_changed.insert(env.name.clone());

                promoted.push(PromoteResult {
                    topic: topic.clone(),
                    env: (*env).clone(),
                    rebuild: None,
                    dry_run: false,
                });
            }
        }

        // Rebuild each changed env exactly once and backfill the rebuild into promoted entries
        for env_id in &envs_needing_rebuild {
            let rebuild = rebuild_service::rebuild_env(conn, env_id, this_repo_path, false, false)?;
            // Attach the rebuild result to every PromoteResult for this env
            for pr in promoted.iter_mut() {
                if pr.env.id == *env_id {
                    pr.rebuild = Some(rebuild.clone());
                }
            }
        }
    }

    Ok(AutoPromoteResult {
        refreshed_topics: refreshed_count,
        promoted,
        envs_changed: envs_changed.into_iter().collect(),
    })
}
