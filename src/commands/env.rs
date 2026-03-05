use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::{env_service, repo_service};
use crate::error::Result;
use crate::id::RepoId;
use crate::types::Environment;

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
        /// List environments across all tracked repos
        #[arg(long)]
        all_repos: bool,
    },
    /// Show environment status
    Status {
        /// Environment ID
        id: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiRepoEnvs {
    repo_name: String,
    repo_id: RepoId,
    environments: Vec<Environment>,
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
            let repo_id: RepoId = repo
                .parse()
                .map_err(|_| crate::error::RestackError::RepoNotFound(RepoId::new()))?;
            let env = env_service::add_env(conn, &repo_id, name, branch, *ordinal, *auto_promote)?;
            Ok(serde_json::to_string_pretty(&env)?)
        }
        EnvCommand::List { repo, all_repos } => {
            if *all_repos {
                let repos = repo_service::list_repos(conn)?;
                let mut results = Vec::new();
                for r in &repos {
                    let environments = env_service::list_envs(conn, Some(&r.id))?;
                    results.push(MultiRepoEnvs {
                        repo_name: r.name.clone(),
                        repo_id: r.id.clone(),
                        environments,
                    });
                }
                Ok(serde_json::to_string_pretty(&results)?)
            } else {
                let repo_id = repo
                    .as_ref()
                    .map(|r| r.parse::<RepoId>())
                    .transpose()
                    .map_err(|_| crate::error::RestackError::RepoNotFound(RepoId::new()))?;
                let envs = env_service::list_envs(conn, repo_id.as_ref())?;
                Ok(serde_json::to_string_pretty(&envs)?)
            }
        }
        EnvCommand::Status { id } => {
            let env_id = id
                .parse()
                .map_err(|_| crate::error::RestackError::EnvNotFound(crate::id::EnvId::new()))?;
            let status = env_service::get_env_status(conn, &env_id)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
    }
}
