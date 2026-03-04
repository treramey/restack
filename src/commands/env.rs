use clap::Subcommand;
use rusqlite::Connection;

use crate::core::env_service;
use crate::error::Result;
use crate::id::RepoId;

#[derive(Subcommand)]
pub enum EnvCommand {
    /// Add an environment
    Add {
        /// Environment name
        name: String,
        /// Branch name for this environment
        #[arg(long)]
        branch: String,
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Sort ordinal (lower = rebuilt first)
        #[arg(long, default_value = "0")]
        ordinal: i32,
        /// Auto-promote topics to this env
        #[arg(long)]
        auto_promote: bool,
    },
    /// List environments
    List {
        /// Filter by repo ID
        #[arg(long)]
        repo: Option<String>,
    },
    /// Show environment status
    Status {
        /// Environment ID
        id: String,
    },
}

pub fn handle(conn: &Connection, cmd: &EnvCommand) -> Result<String> {
    match cmd {
        EnvCommand::Add {
            name,
            branch,
            repo,
            ordinal,
            auto_promote,
        } => {
            let repo_id: RepoId = repo.parse().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let env = env_service::add_env(conn, &repo_id, name, branch, *ordinal, *auto_promote)?;
            Ok(serde_json::to_string_pretty(&env)?)
        }
        EnvCommand::List { repo } => {
            let repo_id = repo.as_ref().map(|r| r.parse::<RepoId>()).transpose().map_err(|_| {
                crate::error::RestackError::RepoNotFound(RepoId::new())
            })?;
            let envs = env_service::list_envs(conn, repo_id.as_ref())?;
            Ok(serde_json::to_string_pretty(&envs)?)
        }
        EnvCommand::Status { id } => {
            let env_id = id.parse().map_err(|_| {
                crate::error::RestackError::EnvNotFound(crate::id::EnvId::new())
            })?;
            let status = env_service::get_env_status(conn, &env_id)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
    }
}
