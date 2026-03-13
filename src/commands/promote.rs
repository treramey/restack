use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{env_sync_service, promote_service, repo_service};
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

pub fn handle(
    conn: &Connection,
    cmd: &PromoteCommand,
    repo_path: &Path,
    no_reconcile: bool,
) -> Result<String> {
    match cmd {
        PromoteCommand::To {
            topic,
            env,
            repo,
            dry_run,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), repo_path)?;
            if !no_reconcile {
                let r_path = std::path::Path::new(&repo.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo.id, r_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }
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
            if !no_reconcile {
                let r_path = std::path::Path::new(&repo.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo.id, r_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }
            let result =
                promote_service::demote_from(conn, topic, env, &repo.id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
