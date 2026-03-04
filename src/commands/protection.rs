use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{protection_service, provider_service};
use crate::error::Result;

#[derive(Subcommand)]
pub enum ProtectionCommand {
    /// Set branch protection rules
    Set {
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Branch to protect
        #[arg(long)]
        branch: String,
        /// Required CI checks (comma-separated)
        #[arg(long, value_delimiter = ',')]
        checks: Vec<String>,
        /// Require pull request reviews
        #[arg(long)]
        require_pr: bool,
        /// Minimum number of approvals
        #[arg(long, default_value = "1")]
        min_approvals: u32,
    },
    /// Protect all environment branches
    Envs {
        /// Repo ID
        #[arg(long)]
        repo: String,
    },
}

pub fn handle(conn: &Connection, cmd: &ProtectionCommand) -> Result<String> {
    match cmd {
        ProtectionCommand::Set {
            repo,
            branch,
            checks,
            require_pr,
            min_approvals,
        } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let result = protection_service::set_branch_protection(
                &repo,
                branch,
                checks,
                *require_pr,
                *min_approvals,
            )?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        ProtectionCommand::Envs { repo } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let results = protection_service::protect_env_branches(conn, &repo)?;
            Ok(serde_json::to_string_pretty(&results)?)
        }
    }
}
