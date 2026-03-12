use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::repo_service;
use crate::error::Result;

#[derive(Subcommand)]
pub enum RepoCommand {
    /// List all repositories
    List,
    /// Add a repository to the workspace
    Add {
        /// Path to the repository
        path: String,
        /// Optional name for the repository (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
        /// Optional repo ID override
        #[arg(short, long)]
        id: Option<String>,
        /// Discover topics immediately after adding
        #[arg(long)]
        discover: bool,
    },
    /// Auto-detect and add repositories in the workspace
    Detect,
    /// Remove a repository from the workspace
    Remove {
        /// Repo ID or name
        id: String,
    },
}

pub fn handle(conn: &Connection, cmd: &RepoCommand, workspace_root: &Path) -> Result<String> {
    match cmd {
        RepoCommand::List => {
            let repos = repo_service::list_repos(conn)?;
            Ok(serde_json::to_string_pretty(&repos)?)
        }
        RepoCommand::Add {
            path,
            name,
            id: _,
            discover,
        } => {
            let result =
                repo_service::add_repo(conn, workspace_root, path, name.as_deref(), *discover)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        RepoCommand::Detect => {
            let result = repo_service::detect_repos(conn, workspace_root)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        RepoCommand::Remove { id } => {
            repo_service::remove_repo(conn, id)?;
            Ok(serde_json::json!({ "deleted": true }).to_string())
        }
    }
}
