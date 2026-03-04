use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::repo_service;
use crate::error::Result;

#[derive(Subcommand)]
pub enum RepoCommand {
    /// Add a repository to the workspace
    Add {
        /// Path to the git repository
        path: String,
        /// Optional name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a repository from the workspace
    Remove {
        /// Repo ID or name
        id: String,
    },
    /// List all repositories
    List,
    /// Auto-discover git repos in workspace subdirectories
    Detect,
}

pub fn handle(conn: &Connection, cmd: &RepoCommand, workspace_root: &Path) -> Result<String> {
    match cmd {
        RepoCommand::Add { path, name } => {
            let repo = repo_service::add_repo(conn, workspace_root, path, name.as_deref())?;
            Ok(serde_json::to_string_pretty(&repo)?)
        }
        RepoCommand::Remove { id } => {
            repo_service::remove_repo(conn, id)?;
            Ok(serde_json::json!({ "deleted": true }).to_string())
        }
        RepoCommand::List => {
            let repos = repo_service::list_repos(conn)?;
            Ok(serde_json::to_string_pretty(&repos)?)
        }
        RepoCommand::Detect => {
            let result = repo_service::detect_repos(conn, workspace_root)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
