use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::promote_service;
use crate::error::Result;
use crate::id::RepoId;

#[derive(Subcommand)]
pub enum PromoteCommand {
    /// Promote a topic to an environment
    To {
        /// Topic ID or branch name
        topic: String,
        /// Target environment name
        env: String,
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Demote a topic from an environment
    From {
        /// Topic ID or branch name
        topic: String,
        /// Environment name to remove from
        env: String,
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn handle(conn: &Connection, cmd: &PromoteCommand, repo_path: &Path) -> Result<String> {
    match cmd {
        PromoteCommand::To {
            topic,
            env,
            repo,
            dry_run,
        } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let result =
                promote_service::promote_to(conn, topic, env, &repo_id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        PromoteCommand::From {
            topic,
            env,
            repo,
            dry_run,
        } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let result =
                promote_service::demote_from(conn, topic, env, &repo_id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
