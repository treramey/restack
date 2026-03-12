use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::core::{discovery_service, repo_service, topic_service};
use crate::db::repo_repo;
use crate::error::Result;
use crate::id::RepoId;
use crate::types::Topic;

#[derive(Subcommand)]
pub enum TopicCommand {
    /// Track a branch as a topic
    Track {
        /// Branch name
        branch: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Untrack a topic
    Untrack {
        /// Topic ID or branch name
        id: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Archive a topic (hide from board, mark as closed)
    Archive {
        /// Topic ID or branch name
        id: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Close a topic: delete branch on origin + local, then remove from DB
    Close {
        /// Topic ID or branch name
        id: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// List tracked topics
    List {
        /// Filter by repo ID
        #[arg(long)]
        repo: Option<String>,
        /// List topics across all tracked repos
        #[arg(long)]
        all_repos: bool,
    },
    /// Show topic status
    Status {
        /// Topic ID or branch name
        id: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// List topic-environment assignments
    Envs,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiRepoTopics {
    repo_name: String,
    repo_id: RepoId,
    topics: Vec<Topic>,
}

pub fn handle(conn: &Connection, cmd: &TopicCommand, workspace_root: &Path) -> Result<String> {
    match cmd {
        TopicCommand::Track { branch, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            let topic = topic_service::track_topic(conn, &repo.id, branch)?;
            Ok(serde_json::to_string_pretty(&topic)?)
        }
        TopicCommand::Untrack { id, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            topic_service::untrack_topic(conn, id, &repo.id)?;
            Ok(serde_json::json!({ "deleted": true }).to_string())
        }
        TopicCommand::Close { id, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            let repo_path = std::path::Path::new(&repo.path);
            let topic = crate::db::topic_repo::get_topic_by_branch(conn, &repo.id, id)?
                .or_else(|| {
                    id.parse()
                        .ok()
                        .and_then(|tid| crate::db::topic_repo::get_topic(conn, &tid).ok())
                })
                .ok_or_else(|| {
                    crate::error::RestackError::TopicNotFound(id.parse().unwrap_or_default())
                })?;

            // Delete remote branch (best-effort)
            let _ = crate::git::branch_delete(repo_path, &topic.branch, true);
            // Delete local branch (best-effort, may fail if checked out)
            let _ = crate::git::branch_delete(repo_path, &topic.branch, false);

            // Remove from all environments and delete from DB
            crate::db::topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;
            crate::db::topic_repo::delete_topic(conn, &topic.id)?;

            Ok(serde_json::json!({ "deleted": true, "branch": topic.branch }).to_string())
        }
        TopicCommand::Archive { id, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            let topic = crate::db::topic_repo::get_topic_by_branch(conn, &repo.id, id)?
                .or_else(|| {
                    id.parse()
                        .ok()
                        .and_then(|tid| crate::db::topic_repo::get_topic(conn, &tid).ok())
                })
                .ok_or_else(|| {
                    crate::error::RestackError::TopicNotFound(id.parse().unwrap_or_default())
                })?;
            let archived = discovery_service::archive_topic(conn, &topic.id)?;
            Ok(serde_json::to_string_pretty(&archived)?)
        }
        TopicCommand::List { repo, all_repos } => {
            // Auto-discover new branches before listing
            let config_path = workspace_root.join(".restack/config.toml");
            let cfg = if config_path.exists() {
                config::load_config(&config_path)?
            } else {
                config::default_config()
            };

            let repos_to_discover = if let Some(r) = repo {
                vec![repo_service::resolve_repo(conn, Some(r), workspace_root)?]
            } else {
                repo_repo::list_repos(conn).unwrap_or_default()
            };

            for r in &repos_to_discover {
                let repo_path = std::path::Path::new(&r.path);
                let _ = discovery_service::discover_topics(conn, &r.id, repo_path, &cfg);
            }

            if *all_repos {
                let repos = repo_service::list_repos(conn)?;
                let mut results = Vec::new();
                for r in &repos {
                    let topics = topic_service::list_topics(conn, Some(&r.id))?;
                    results.push(MultiRepoTopics {
                        repo_name: r.name.clone(),
                        repo_id: r.id.clone(),
                        topics,
                    });
                }
                Ok(serde_json::to_string_pretty(&results)?)
            } else {
                let repo_id = repo
                    .as_ref()
                    .map(|r| {
                        Ok::<_, crate::error::RestackError>(
                            repo_service::resolve_repo(conn, Some(r), workspace_root)?.id,
                        )
                    })
                    .transpose()?;
                let topics = topic_service::list_topics(conn, repo_id.as_ref())?;
                Ok(serde_json::to_string_pretty(&topics)?)
            }
        }
        TopicCommand::Status { id, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            let status = topic_service::get_topic_status(conn, id, &repo.id)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
        TopicCommand::Envs => {
            let topic_envs = crate::db::topic_env_repo::list_all_topic_environments(conn)?;
            Ok(serde_json::to_string_pretty(&topic_envs)?)
        }
    }
}
