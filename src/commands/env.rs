use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::{env_init_service, env_service, repo_service};
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
    /// Initialize integration environments (create branches + register)
    Init {
        /// Repo ID (auto-resolved if single repo in workspace)
        #[arg(long)]
        repo: Option<String>,
        /// Interactive mode: select branches from local/remote
        #[arg(long, short)]
        interactive: bool,
        /// Push newly created branches to remote
        #[arg(long)]
        push: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiRepoEnvs {
    repo_name: String,
    repo_id: RepoId,
    environments: Vec<Environment>,
}

pub fn handle(conn: &Connection, cmd: &EnvCommand, cwd: &Path) -> Result<String> {
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
        EnvCommand::Init {
            repo,
            interactive,
            push,
        } => handle_init(conn, cwd, repo.as_deref(), *interactive, *push),
    }
}

fn handle_init(
    conn: &Connection,
    cwd: &Path,
    repo_arg: Option<&str>,
    interactive: bool,
    push: bool,
) -> Result<String> {
    let repo = env_init_service::resolve_repo(conn, repo_arg)?;
    let repo_path = std::path::PathBuf::from(&repo.path);

    let config_path = cwd.join(".restack").join("config.toml");
    let cfg = if config_path.exists() {
        crate::config::load_config(&config_path)?
    } else {
        crate::config::default_config()
    };

    let envs = if interactive {
        prompt_interactive_envs(&repo_path)?
    } else {
        env_init_service::envs_from_config(&cfg)
    };

    if envs.is_empty() {
        return Ok(serde_json::to_string_pretty(
            &serde_json::json!({"message": "No environments to initialize."}),
        )?);
    }

    let result = env_init_service::init_envs(
        conn,
        &repo.id,
        &repo_path,
        &cfg.defaults.base_branch,
        &envs,
        push,
    )?;

    for warning in &result.warnings {
        eprintln!("Warning: {}", warning);
    }

    Ok(serde_json::to_string_pretty(&result)?)
}

fn prompt_interactive_envs(
    repo_path: &Path,
) -> Result<Vec<env_init_service::EnvInitInput>> {
    use dialoguer::{Confirm, Input, MultiSelect};

    let all_branches = crate::git::list_all_branches(repo_path)?;
    if all_branches.is_empty() {
        return Ok(Vec::new());
    }

    let display_names: Vec<String> = all_branches
        .iter()
        .map(|(name, is_local)| {
            if *is_local {
                name.clone()
            } else {
                format!("{} (remote)", name)
            }
        })
        .collect();

    let selections = MultiSelect::new()
        .with_prompt("Select integration branches")
        .items(&display_names)
        .interact()
        .map_err(|e| std::io::Error::other(e))?;

    if selections.is_empty() {
        return Ok(Vec::new());
    }

    let mut envs = Vec::new();
    for (i, &idx) in selections.iter().enumerate() {
        let branch = &all_branches[idx].0;

        let name: String = Input::new()
            .with_prompt(format!("Environment name for '{}'", branch))
            .default(branch.clone())
            .interact_text()
            .map_err(|e| std::io::Error::other(e))?;

        let ordinal: i32 = Input::new()
            .with_prompt("Sort ordinal (lower = rebuilt first)")
            .default(i as i32)
            .interact_text()
            .map_err(|e| std::io::Error::other(e))?;

        let auto_promote = Confirm::new()
            .with_prompt("Auto-promote topics?")
            .default(false)
            .interact()
            .map_err(|e| std::io::Error::other(e))?;

        envs.push(env_init_service::EnvInitInput {
            name,
            branch: branch.clone(),
            ordinal,
            auto_promote,
        });
    }

    eprintln!("\nEnvironments to create:");
    for env in &envs {
        eprintln!(
            "  {} -> branch '{}' (ordinal: {}, auto-promote: {})",
            env.name, env.branch, env.ordinal, env.auto_promote
        );
    }

    let confirmed = Confirm::new()
        .with_prompt("Create these environments?")
        .default(true)
        .interact()
        .map_err(|e| std::io::Error::other(e))?;

    if confirmed {
        Ok(envs)
    } else {
        Ok(Vec::new())
    }
}
