use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config::WorkspaceConfig;
use crate::db::{env_repo, topic_repo};
use crate::error::Result;
use crate::git;
use crate::id::RepoId;
use crate::types::{Topic, TopicStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub discovered: i32,
    pub created: i32,
    pub closed: i32,
    pub skipped: i32,
    pub topics: Vec<Topic>,
}

pub fn discover_topics(
    conn: &Connection,
    repo_id: &RepoId,
    repo_path: &Path,
    config: &WorkspaceConfig,
) -> Result<DiscoveryResult> {
    if git::has_remote(repo_path) {
        let _ = git::fetch(repo_path);
    }

    let envs = env_repo::list_envs(conn, Some(repo_id))?;
    let env_branches: Vec<&str> = envs.iter().map(|e| e.branch.as_str()).collect();

    let all_branches = git::list_all_branches(repo_path)?;

    let mut discovered = 0i32;
    let mut created = 0i32;
    let mut skipped = 0i32;
    let mut new_topics = Vec::new();

    for (branch, _is_local) in all_branches {
        if config.discovery.should_exclude(&branch, &env_branches) {
            skipped += 1;
            continue;
        }

        discovered += 1;

        if topic_repo::get_topic_by_branch(conn, repo_id, &branch)?.is_none() {
            let topic = topic_repo::create_topic(conn, repo_id, &branch, None, None)?;
            new_topics.push(topic);
            created += 1;
        }
    }

    let closed = close_deleted_topics(conn, repo_id, repo_path)?;

    let topics = topic_repo::list_topics(conn, Some(repo_id))?;

    Ok(DiscoveryResult {
        discovered,
        created,
        closed,
        skipped,
        topics,
    })
}

fn close_deleted_topics(conn: &Connection, repo_id: &RepoId, repo_path: &Path) -> Result<i32> {
    let topics = topic_repo::list_topics(conn, Some(repo_id))?;
    let mut closed = 0i32;

    for topic in topics {
        if topic.status == TopicStatus::Closed || topic.status == TopicStatus::Graduated {
            continue;
        }

        let local_exists = git::branch_exists(repo_path, &topic.branch)?;
        let remote_exists = git::remote_branch_exists(repo_path, &topic.branch)?;

        if !local_exists && !remote_exists {
            topic_repo::update_topic_status(conn, &topic.id, TopicStatus::Closed)?;
            closed += 1;
        }
    }

    Ok(closed)
}

pub fn archive_topic(conn: &Connection, topic_id: &crate::id::TopicId) -> Result<Topic> {
    topic_repo::update_topic_status_return(conn, topic_id, TopicStatus::Closed)
}
