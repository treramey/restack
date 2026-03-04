use rusqlite::Connection;

use crate::db::env_repo;
use crate::error::Result;
use crate::provider::{self, BranchProtectionParams, BranchProtectionResult};
use crate::types::Repo;

pub fn set_branch_protection(
    repo: &Repo,
    branch: &str,
    required_checks: &[String],
    require_pr: bool,
    min_approvals: u32,
) -> Result<BranchProtectionResult> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let params = BranchProtectionParams {
        branch: branch.to_string(),
        required_checks: required_checks.to_vec(),
        require_pr,
        min_approvals,
    };
    adapter.set_branch_protection(&params)
}

pub fn protect_env_branches(
    conn: &Connection,
    repo: &Repo,
) -> Result<Vec<BranchProtectionResult>> {
    let envs = env_repo::list_envs(conn, Some(&repo.id))?;
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let mut results = Vec::new();

    for env in &envs {
        let params = BranchProtectionParams {
            branch: env.branch.clone(),
            required_checks: Vec::new(),
            require_pr: true,
            min_approvals: 1,
        };
        let result = adapter.set_branch_protection(&params)?;
        results.push(result);
    }

    Ok(results)
}
