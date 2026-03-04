use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::rebuild_service;
use crate::error::Result;
use crate::id::{EnvId, RepoId};

#[derive(Subcommand)]
pub enum RebuildCommand {
    /// Rebuild a specific environment
    Env {
        /// Environment ID
        env: String,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Rebuild all environments for a repo
    All {
        /// Repo ID
        repo: String,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn handle(conn: &Connection, cmd: &RebuildCommand, repo_path: &Path) -> Result<String> {
    match cmd {
        RebuildCommand::Env { env, dry_run } => {
            let env_id: EnvId = env.parse().map_err(|_| {
                crate::error::RestackError::EnvNotFound(EnvId::new())
            })?;
            let rebuild = rebuild_service::rebuild_env(conn, &env_id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&rebuild)?)
        }
        RebuildCommand::All { repo, dry_run } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let rebuilds =
                rebuild_service::rebuild_all(conn, &repo_id, repo_path, *dry_run)?;
            Ok(serde_json::to_string_pretty(&rebuilds)?)
        }
    }
}
