use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::rebuild_service;
use crate::db::{conflict_repo, env_repo, rebuild_repo, repo_repo, topic_env_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::id::RepoId;
use crate::types::{Conflict, Environment, Rebuild, Topic};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromoteResult {
    pub topic: Topic,
    pub env: Environment,
    pub rebuild: Option<Rebuild>,
    pub conflicts: Vec<Conflict>,
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

        // Debounce: skip rebuild if last one completed within the configured window
        // TODO: load debounce_secs from config when available in this context
        let debounce_secs = 0u32;
        let should_skip = debounce_secs > 0 && {
            if let Ok(Some(last)) = rebuild_repo::get_last_rebuild(conn, &env.id) {
                if let Some(completed) = last.completed_at {
                    let elapsed = chrono::Utc::now() - completed;
                    elapsed.num_seconds() < debounce_secs as i64
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_skip {
            return Ok(PromoteResult {
                topic,
                env,
                rebuild: None,
                conflicts: Vec::new(),
                dry_run,
            });
        }

        // Trigger rebuild
        let rebuild = rebuild_service::rebuild_env(conn, &env.id, this_repo_path, false, false)?;
        let conflicts = conflict_repo::list_conflicts(conn, &rebuild.id)?;

        Ok(PromoteResult {
            topic,
            env,
            rebuild: Some(rebuild),
            conflicts,
            dry_run,
        })
    } else {
        Ok(PromoteResult {
            topic,
            env,
            rebuild: None,
            conflicts: Vec::new(),
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
        let conflicts = conflict_repo::list_conflicts(conn, &rebuild.id)?;

        Ok(PromoteResult {
            topic,
            env,
            rebuild: Some(rebuild),
            conflicts,
            dry_run,
        })
    } else {
        Ok(PromoteResult {
            topic,
            env,
            rebuild: None,
            conflicts: Vec::new(),
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
