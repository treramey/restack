use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{pipeline_service, provider_service};
use crate::error::Result;

#[derive(Subcommand)]
pub enum PipelineCommand {
    /// Trigger a CI pipeline
    Trigger {
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Branch to build
        #[arg(long)]
        branch: String,
        /// Pipeline/workflow name
        #[arg(long)]
        name: Option<String>,
    },
}

pub fn handle(conn: &Connection, cmd: &PipelineCommand) -> Result<String> {
    match cmd {
        PipelineCommand::Trigger {
            repo,
            branch,
            name,
        } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let result = pipeline_service::trigger_pipeline(&repo, branch, name.as_deref())?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
