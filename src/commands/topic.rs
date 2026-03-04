use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{provider_service, topic_service};
use crate::error::Result;
use crate::id::RepoId;

#[derive(Subcommand)]
pub enum TopicCommand {
    /// Track a branch as a topic
    Track {
        /// Branch name
        branch: String,
        /// Repo ID
        #[arg(long)]
        repo: String,
    },
    /// Untrack a topic
    Untrack {
        /// Topic ID or branch name
        id: String,
        /// Repo ID
        #[arg(long)]
        repo: String,
    },
    /// List tracked topics
    List {
        /// Filter by repo ID
        #[arg(long)]
        repo: Option<String>,
    },
    /// Show topic status
    Status {
        /// Topic ID or branch name
        id: String,
        /// Repo ID
        #[arg(long)]
        repo: String,
    },
    /// Sync topics from pull requests
    Sync {
        /// Repo ID
        #[arg(long)]
        repo: String,
    },
}

pub fn handle(conn: &Connection, cmd: &TopicCommand) -> Result<String> {
    match cmd {
        TopicCommand::Track { branch, repo } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let topic = topic_service::track_topic(conn, &repo_id, branch)?;
            Ok(serde_json::to_string_pretty(&topic)?)
        }
        TopicCommand::Untrack { id, repo } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            topic_service::untrack_topic(conn, id, &repo_id)?;
            Ok(serde_json::json!({ "deleted": true }).to_string())
        }
        TopicCommand::List { repo } => {
            let repo_id = repo.as_ref().map(|r| r.parse::<RepoId>()).transpose().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let topics = topic_service::list_topics(conn, repo_id.as_ref())?;
            Ok(serde_json::to_string_pretty(&topics)?)
        }
        TopicCommand::Status { id, repo } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let status = topic_service::get_topic_status(conn, id, &repo_id)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
        TopicCommand::Sync { repo } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let result = provider_service::sync_topics_from_prs(conn, &repo)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
