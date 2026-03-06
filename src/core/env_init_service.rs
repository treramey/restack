use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::WorkspaceConfig;
use crate::core::{env_service, repo_service};
use crate::db::{env_repo, repo_repo};
use crate::error::{RestackError, Result};
use crate::git;
use crate::id::RepoId;
use crate::types::Environment;

#[derive(Debug, Clone)]
pub struct EnvInitInput {
    pub name: String,
    pub branch: String,
    pub ordinal: i32,
    pub auto_promote: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvInitResult {
    pub created_branches: Vec<String>,
    pub created_envs: Vec<String>,
    pub skipped_branches: Vec<String>,
    pub skipped_envs: Vec<String>,
    pub pushed_branches: Vec<String>,
    pub warnings: Vec<String>,
    pub environments: Vec<Environment>,
}

pub fn resolve_repo(conn: &Connection, repo_arg: Option<&str>) -> Result<crate::types::Repo> {
    let repo_id = match repo_arg {
        Some(r) => r
            .parse::<RepoId>()
            .map_err(|_| RestackError::InvalidId(r.to_string()))?,
        None => {
            let repos = repo_service::list_repos(conn)?;
            match repos.as_slice() {
                [single] => single.id.clone(),
                [] => return Err(RestackError::NoRepos),
                _ => return Err(RestackError::MultipleRepos),
            }
        }
    };
    Ok(repo_repo::get_repo(conn, &repo_id)?)
}

pub fn envs_from_config(cfg: &WorkspaceConfig) -> Vec<EnvInitInput> {
    let mut envs: Vec<_> = cfg
        .environments
        .iter()
        .map(|(name, ec)| EnvInitInput {
            name: name.clone(),
            branch: ec.branch.clone(),
            ordinal: ec.ordinal,
            auto_promote: ec.auto_promote,
        })
        .collect();

    envs.sort_by_key(|e| e.ordinal);
    envs
}

pub fn init_envs(
    conn: &Connection,
    repo_id: &RepoId,
    repo_path: &Path,
    base_ref: &str,
    envs: &[EnvInitInput],
    push: bool,
) -> Result<EnvInitResult> {
    let mut result = EnvInitResult {
        created_branches: Vec::new(),
        created_envs: Vec::new(),
        skipped_branches: Vec::new(),
        skipped_envs: Vec::new(),
        pushed_branches: Vec::new(),
        warnings: Vec::new(),
        environments: Vec::new(),
    };

    let mut created_branch_names: Vec<String> = Vec::new();

    // Wrap all DB operations in a transaction for atomicity
    let tx = conn.unchecked_transaction()?;

    for input in envs {
        // Git branch creation (no HEAD switch)
        match git::branch_exists(repo_path, &input.branch) {
            Ok(true) => {
                result.skipped_branches.push(input.branch.clone());
            }
            Ok(false) => {
                if let Err(e) = git::branch_create_at(repo_path, &input.branch, base_ref) {
                    rollback_branches(repo_path, &created_branch_names, &mut result.warnings);
                    // tx is dropped without commit, auto-rolling back DB
                    return Err(e.into());
                }
                created_branch_names.push(input.branch.clone());
                result.created_branches.push(input.branch.clone());

                if let Ok(remote_branches) = git::list_remote_branches(repo_path) {
                    if remote_branches.contains(&input.branch) {
                        result.warnings.push(format!(
                            "'{}' exists on remote but not locally — created new local branch from '{}'",
                            input.branch, base_ref
                        ));
                    }
                }
            }
            Err(e) => {
                rollback_branches(repo_path, &created_branch_names, &mut result.warnings);
                return Err(e.into());
            }
        }

        // DB registration (idempotent, within transaction)
        match env_repo::get_env_by_name(&tx, repo_id, &input.name)? {
            Some(existing) => {
                result.skipped_envs.push(input.name.clone());
                result.environments.push(existing);
            }
            None => {
                match env_service::add_env(
                    &tx,
                    repo_id,
                    &input.name,
                    &input.branch,
                    input.ordinal,
                    input.auto_promote,
                ) {
                    Ok(env) => {
                        result.created_envs.push(input.name.clone());
                        result.environments.push(env);
                    }
                    Err(e) => {
                        // tx is dropped without commit, auto-rolling back DB
                        rollback_branches(repo_path, &created_branch_names, &mut result.warnings);
                        return Err(e);
                    }
                }
            }
        }
    }

    // Commit DB transaction only after all branches + envs succeed
    tx.commit()?;

    // Push is best-effort, after atomic create
    if push {
        for branch in &result.created_branches {
            match git::push(repo_path, branch) {
                Ok(()) => {
                    result.pushed_branches.push(branch.clone());
                }
                Err(e) => {
                    result
                        .warnings
                        .push(format!("Failed to push '{}': {}", branch, e));
                }
            }
        }
    }

    Ok(result)
}

fn rollback_branches(repo_path: &Path, branches: &[String], warnings: &mut Vec<String>) {
    for branch in branches {
        if let Err(e) = git::branch_delete(repo_path, branch, false) {
            warnings.push(format!("Rollback failed for branch '{}': {}", branch, e));
        }
    }
}
