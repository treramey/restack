use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::repo_service;
use crate::error::Result;

#[derive(Subcommand)]
pub enum RepoCommand {
    /// List all repositories
    List,
    /// Remove a repository from the workspace
    Remove {
        /// Repo ID or name
        id: String,
    },
}

pub fn handle(conn: &Connection, cmd: &RepoCommand, _workspace_root: &Path) -> Result<String> {
    match cmd {
        RepoCommand::List => {
            let repos = repo_service::list_repos(conn)?;
            Ok(serde_json::to_string_pretty(&repos)?)
        }
        RepoCommand::Remove { id } => {
            repo_service::remove_repo(conn, id)?;
            Ok(serde_json::json!({ "deleted": true }).to_string())
        }
    }
}
