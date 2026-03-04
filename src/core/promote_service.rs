use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::rebuild_service;
use crate::db::{env_repo, topic_env_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::id::RepoId;
use crate::types::{Environment, Rebuild, Topic};

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
    repo_path: &Path,
    dry_run: bool,
) -> Result<PromoteResult> {
    let topic = resolve_topic(conn, topic_id_or_branch, repo_id)?;
    let env = resolve_env(conn, env_name, repo_id)?;

    if !dry_run {
        // Add topic to environment
        topic_env_repo::add_topic_to_env(conn, &topic.id, &env.id)?;

        // Trigger rebuild
        let rebuild = rebuild_service::rebuild_env(conn, &env.id, repo_path, false)?;
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
    repo_path: &Path,
    dry_run: bool,
) -> Result<PromoteResult> {
    let topic = resolve_topic(conn, topic_id_or_branch, repo_id)?;
    let env = resolve_env(conn, env_name, repo_id)?;

    if !dry_run {
        // Remove topic from environment
        topic_env_repo::remove_topic_from_env(conn, &topic.id, &env.id)?;

        // Trigger rebuild
        let rebuild = rebuild_service::rebuild_env(conn, &env.id, repo_path, false)?;
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
        None => Err(RestackError::EnvNotFound(
            name.parse().unwrap_or_default(),
        )),
    }
}
