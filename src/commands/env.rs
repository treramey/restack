use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::{env_init_service, env_service, env_sync_service, repo_service};
use crate::db::{env_repo, rebuild_repo, repo_repo};
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
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
        /// Sort ordinal (lower = rebuilt first)
        #[arg(long, default_value = "0")]
        ordinal: i32,
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
    /// Override CI status for an environment (force-clear a failed/pending state)
    CiOverride {
        /// Environment name
        env_name: String,
        /// Repo ID (auto-resolved if single repo in workspace)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Blame CI failure: identify the likely culprit topic
    Blame {
        /// Environment name
        env_name: String,
        /// Repo ID (auto-resolved if single repo in workspace)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Show speculative CI status for an environment
    SpeculativeStatus {
        /// Environment name
        env_name: String,
        /// Repo ID (auto-resolved if single repo in workspace)
        #[arg(long)]
        repo: Option<String>,
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

pub fn handle(
    conn: &Connection,
    cmd: &EnvCommand,
    cwd: &Path,
    no_reconcile: bool,
) -> Result<String> {
    match cmd {
        EnvCommand::Add {
            name,
            branch,
            repo,
            ordinal,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), cwd)?;
            let env = env_service::add_env(conn, &repo.id, name, branch, *ordinal)?;
            Ok(serde_json::to_string_pretty(&env)?)
        }
        EnvCommand::List { repo, all_repos } => {
            if *all_repos {
                if !no_reconcile {
                    let repos = repo_repo::list_repos(conn)?;
                    for r in &repos {
                        let repo_path = Path::new(&r.path);
                        if let Some(summary) =
                            env_sync_service::maybe_reconcile_repo_envs(conn, &r.id, repo_path)?
                        {
                            eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                        }
                    }
                }
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
                let repo = repo_service::resolve_repo(conn, repo.as_deref(), cwd)?;
                if !no_reconcile {
                    let repo_path = Path::new(&repo.path);
                    if let Some(summary) =
                        env_sync_service::maybe_reconcile_repo_envs(conn, &repo.id, repo_path)?
                    {
                        eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                    }
                }
                let envs = env_service::list_envs(conn, Some(&repo.id))?;
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
        EnvCommand::CiOverride { env_name, repo } => {
            let repo_obj = repo_service::resolve_repo(conn, repo.as_deref(), cwd)?;
            if !no_reconcile {
                let repo_path = Path::new(&repo_obj.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo_obj.id, repo_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }
            let env = match env_repo::get_env_by_name(conn, &repo_obj.id, env_name)? {
                Some(e) => e,
                None => {
                    return Err(crate::error::RestackError::EnvNotFound(
                        env_name.parse().unwrap_or_default(),
                    ))
                }
            };
            let rebuild = match rebuild_repo::get_last_rebuild(conn, &env.id)? {
                Some(r) => r,
                None => {
                    return Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "message": "No rebuild found for environment"
                    }))?)
                }
            };
            conn.execute(
                "UPDATE rebuilds SET ci_override = 'passed' WHERE id = ?1",
                rusqlite::params![rebuild.id],
            )?;
            env_repo::set_env_ci_status(conn, &env.id, None, None)?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "message": "CI override applied",
                "env_id": env.id,
                "env_name": env.name,
                "rebuild_id": rebuild.id,
            }))?)
        }
        EnvCommand::Blame { env_name, repo } => {
            let repo_obj = repo_service::resolve_repo(conn, repo.as_deref(), cwd)?;
            if !no_reconcile {
                let repo_path = Path::new(&repo_obj.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo_obj.id, repo_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }
            let env = match env_repo::get_env_by_name(conn, &repo_obj.id, env_name)? {
                Some(e) => e,
                None => {
                    return Err(crate::error::RestackError::EnvNotFound(
                        env_name.parse().unwrap_or_default(),
                    ))
                }
            };
            let result = crate::core::speculative_ci_service::speculative_blame_or_fallback(
                conn, &env.id, &repo_obj,
            )?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        EnvCommand::SpeculativeStatus { env_name, repo } => {
            let repo_obj = repo_service::resolve_repo(conn, repo.as_deref(), cwd)?;
            if !no_reconcile {
                let repo_path = Path::new(&repo_obj.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo_obj.id, repo_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }
            let env = match env_repo::get_env_by_name(conn, &repo_obj.id, env_name)? {
                Some(e) => e,
                None => {
                    return Err(crate::error::RestackError::EnvNotFound(
                        env_name.parse().unwrap_or_default(),
                    ))
                }
            };
            let result = crate::core::speculative_ci_service::check_speculative_ci(
                conn, &env.id, &repo_obj,
            )?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        EnvCommand::Init {
            repo,
            interactive,
            push,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), cwd)?;
            handle_init(conn, cwd, &repo, *interactive, *push)
        }
    }
}

fn handle_init(
    conn: &Connection,
    cwd: &Path,
    repo: &crate::types::Repo,
    interactive: bool,
    push: bool,
) -> Result<String> {
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

fn prompt_interactive_envs(repo_path: &Path) -> Result<Vec<env_init_service::EnvInitInput>> {
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

        envs.push(env_init_service::EnvInitInput {
            name,
            branch: branch.clone(),
            ordinal,
        });
    }

    eprintln!("\nEnvironments to create:");
    for env in &envs {
        eprintln!(
            "  {} -> branch '{}' (ordinal: {})",
            env.name, env.branch, env.ordinal
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
