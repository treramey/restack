use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{pr_service, provider_service};
use crate::error::Result;
use crate::provider::MergeStrategy;

#[derive(Subcommand)]
pub enum PrCommand {
    /// Create a pull request
    Create {
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Source branch (head)
        #[arg(long)]
        head: String,
        /// Target branch (base)
        #[arg(long)]
        base: String,
        /// PR title
        #[arg(long)]
        title: String,
        /// PR body/description
        #[arg(long)]
        body: Option<String>,
        /// Create as draft
        #[arg(long)]
        draft: bool,
    },
    /// Merge a pull request
    Merge {
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// PR number
        pr_number: String,
        /// Merge strategy
        #[arg(long, value_enum, default_value = "squash")]
        strategy: MergeStrategyArg,
        /// Delete branch after merge
        #[arg(long)]
        delete_branch: bool,
    },
}

#[derive(Clone, clap::ValueEnum)]
pub enum MergeStrategyArg {
    Merge,
    Squash,
    Rebase,
}

impl From<MergeStrategyArg> for MergeStrategy {
    fn from(arg: MergeStrategyArg) -> Self {
        match arg {
            MergeStrategyArg::Merge => MergeStrategy::Merge,
            MergeStrategyArg::Squash => MergeStrategy::Squash,
            MergeStrategyArg::Rebase => MergeStrategy::Rebase,
        }
    }
}

pub fn handle(conn: &Connection, cmd: &PrCommand) -> Result<String> {
    match cmd {
        PrCommand::Create {
            repo,
            head,
            base,
            title,
            body,
            draft,
        } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let pr = pr_service::create_pr(&repo, head, base, title, body.as_deref(), *draft)?;
            Ok(serde_json::to_string_pretty(&pr)?)
        }
        PrCommand::Merge {
            repo,
            pr_number,
            strategy,
            delete_branch,
        } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let result =
                pr_service::merge_pr(&repo, pr_number, strategy.clone().into(), *delete_branch)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
