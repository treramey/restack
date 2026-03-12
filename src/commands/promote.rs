use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::promote_service;
use crate::core::repo_service;
use crate::error::Result;

#[derive(Subcommand)]
pub enum PromoteCommand {
    /// Promote a topic to an environment
    To {
        /// Topic ID or branch name
        topic: String,
        /// Target environment name
        env: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Auto-promote CI-passed topics to auto_promote environments
    Auto,
    /// Demote a topic from an environment
    From {
        /// Topic ID or branch name
        topic: String,
        /// Environment name to remove from
        env: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn handle(conn: &Connection, cmd: &PromoteCommand, repo_path: &Path) -> Result<String> {
    match cmd {
        PromoteCommand::Auto => {
            let result = promote_service::promote_auto(conn)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        PromoteCommand::To {
            topic,
            env,
            repo,
            dry_run,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), repo_path)?;
            let result =
                promote_service::promote_to(conn, topic, env, &repo.id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        PromoteCommand::From {
            topic,
            env,
            repo,
            dry_run,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), repo_path)?;
            let result =
                promote_service::demote_from(conn, topic, env, &repo.id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
