use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{topic_env_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::id::RepoId;
use crate::types::{Environment, Topic};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicWithEnvs {
    pub topic: Topic,
    pub environments: Vec<Environment>,
}

pub fn track_topic(conn: &Connection, repo_id: &RepoId, branch: &str) -> Result<Topic> {
    // Check not already tracked
    if topic_repo::get_topic_by_branch(conn, repo_id, branch)?.is_some() {
        return Err(RestackError::TopicAlreadyTracked {
            branch: branch.to_string(),
        });
    }

    topic_repo::create_topic(conn, repo_id, branch, None, None)
}

pub fn untrack_topic(conn: &Connection, id_or_branch: &str, repo_id: &RepoId) -> Result<()> {
    let topic = resolve_topic(conn, id_or_branch, repo_id)?;

    // Remove from all environments
    topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;

    // Delete the topic
    topic_repo::delete_topic(conn, &topic.id)
}

pub fn list_topics(conn: &Connection, repo_id: Option<&RepoId>) -> Result<Vec<Topic>> {
    topic_repo::list_topics(conn, repo_id)
}

pub fn get_topic_status(
    conn: &Connection,
    id_or_branch: &str,
    repo_id: &RepoId,
) -> Result<TopicWithEnvs> {
    let topic = resolve_topic(conn, id_or_branch, repo_id)?;

    let env_ids = topic_env_repo::get_envs_for_topic(conn, &topic.id)?;
    let mut environments = Vec::new();
    for env_id in env_ids {
        if let Ok(env) = crate::db::env_repo::get_env(conn, &env_id) {
            environments.push(env);
        }
    }

    Ok(TopicWithEnvs {
        topic,
        environments,
    })
}

/// Resolve a topic by ID or branch name within a repo
fn resolve_topic(conn: &Connection, id_or_branch: &str, repo_id: &RepoId) -> Result<Topic> {
    // Try as ID first
    if let Ok(id) = id_or_branch.parse() {
        if let Ok(topic) = topic_repo::get_topic(conn, &id) {
            return Ok(topic);
        }
    }

    // Try as branch name
    match topic_repo::get_topic_by_branch(conn, repo_id, id_or_branch)? {
        Some(topic) => Ok(topic),
        None => Err(RestackError::TopicNotFound(
            id_or_branch.parse().unwrap_or_default(),
        )),
    }
}
