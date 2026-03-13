use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config::{DiscoveryMode, WorkspaceConfig};
use crate::db::{env_repo, repo_repo, topic_env_repo, topic_repo};
use crate::error::Result;
use crate::git;
use crate::id::RepoId;
use crate::types::{BranchOrigin, Topic, TopicStatus};

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

    let branches = git::list_branch_presence(repo_path)?;

    let mut discovered = 0i32;
    let mut created = 0i32;
    let mut skipped = 0i32;
    let mut new_topics = Vec::new();

    for branch in branches {
        if !should_track_branch(config.discovery.mode, branch.has_local, branch.has_remote) {
            skipped += 1;
            continue;
        }

        if config
            .discovery
            .should_exclude(&branch.branch, &env_branches)
        {
            skipped += 1;
            continue;
        }

        let existing = topic_repo::get_topic_by_branch(conn, repo_id, &branch.branch)?;
        if let Some(topic) = existing {
            let next_origin =
                classify_branch_origin(topic.branch_origin, branch.has_local, branch.has_remote);
            if next_origin != topic.branch_origin {
                topic_repo::update_topic_branch_origin(conn, &topic.id, next_origin)?;
            }
            discovered += 1;
            continue;
        }

        discovered += 1;

        let topic = topic_repo::create_topic(
            conn,
            repo_id,
            &branch.branch,
            classify_new_topic_origin(branch.has_local, branch.has_remote),
            None,
            None,
        )?;
        new_topics.push(topic);
        created += 1;
    }

    let closed = close_deleted_topics(conn, repo_id, repo_path)?;

    // Mark topics already merged into the base branch as graduated.
    // Uses a single `git branch --merged` call instead of per-topic subprocess spawns.
    let repo = repo_repo::get_repo(conn, repo_id)?;
    if let Some(base_ref) = preferred_ref(repo_path, &repo.base_branch) {
        let merged_into_base: std::collections::HashSet<String> =
            git::list_branches_merged_into(repo_path, &base_ref)
                .unwrap_or_default()
                .into_iter()
                .collect();

        let all_topics = topic_repo::list_topics(conn, Some(repo_id))?;
        for topic in &all_topics {
            if topic.status == TopicStatus::Closed || topic.status == TopicStatus::Graduated {
                continue;
            }
            if merged_into_base.contains(&topic.branch) {
                topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;
                topic_repo::update_topic_status(conn, &topic.id, TopicStatus::Graduated)?;
            }
        }
    }

    // Auto-assign active topics to environments based on git containment.
    // One `git branch --merged` call per environment instead of per-topic subprocess spawns.
    if !envs.is_empty() {
        let all_topics = topic_repo::list_topics(conn, Some(repo_id))?;
        let topic_map: std::collections::HashMap<&str, &Topic> =
            all_topics.iter().map(|t| (t.branch.as_str(), t)).collect();

        for env in &envs {
            let env_ref = match preferred_ref(repo_path, &env.branch) {
                Some(r) => r,
                None => continue,
            };
            let merged: std::collections::HashSet<String> =
                git::list_branches_merged_into(repo_path, &env_ref)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();

            for branch_name in &merged {
                let topic = match topic_map.get(branch_name.as_str()) {
                    Some(t) => t,
                    None => continue,
                };
                if topic.status == TopicStatus::Closed || topic.status == TopicStatus::Graduated {
                    continue;
                }
                let existing_envs = topic_env_repo::get_envs_for_topic(conn, &topic.id)?;
                if existing_envs.contains(&env.id) {
                    continue;
                }
                if let Err(e) = topic_env_repo::add_topic_to_env(conn, &topic.id, &env.id) {
                    eprintln!(
                        "Warning: failed to assign topic '{}' to env '{}': {}",
                        topic.branch, env.name, e
                    );
                }
            }
        }
    }

    let topics = topic_repo::list_topics(conn, Some(repo_id))?;

    Ok(DiscoveryResult {
        discovered,
        created,
        closed,
        skipped,
        topics,
    })
}

fn should_track_branch(mode: DiscoveryMode, has_local: bool, has_remote: bool) -> bool {
    match mode {
        DiscoveryMode::OriginOnly => has_remote,
        DiscoveryMode::LocalOnly => has_local && !has_remote,
        DiscoveryMode::All => has_local || has_remote,
    }
}

fn classify_new_topic_origin(has_local: bool, has_remote: bool) -> BranchOrigin {
    if has_remote {
        BranchOrigin::Tracked
    } else if has_local {
        BranchOrigin::LocalOnly
    } else {
        BranchOrigin::Tracked
    }
}

fn classify_branch_origin(
    previous: BranchOrigin,
    has_local: bool,
    has_remote: bool,
) -> BranchOrigin {
    if has_remote {
        BranchOrigin::Tracked
    } else if has_local {
        match previous {
            BranchOrigin::Tracked | BranchOrigin::Orphaned => BranchOrigin::Orphaned,
            BranchOrigin::LocalOnly => BranchOrigin::LocalOnly,
        }
    } else {
        previous
    }
}

/// Resolve a single canonical git ref for a branch name, preferring origin/.
fn preferred_ref(repo: &Path, branch: &str) -> Option<String> {
    if git::remote_branch_exists(repo, branch).unwrap_or(false) {
        Some(format!("refs/remotes/origin/{branch}"))
    } else if git::branch_exists(repo, branch).unwrap_or(false) {
        Some(format!("refs/heads/{branch}"))
    } else {
        None
    }
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
            crate::db::topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;
            topic_repo::delete_topic(conn, &topic.id)?;
            closed += 1;
        }
    }

    Ok(closed)
}

pub fn archive_topic(conn: &Connection, topic_id: &crate::id::TopicId) -> Result<Topic> {
    topic_repo::update_topic_status_return(conn, topic_id, TopicStatus::Closed)
}
