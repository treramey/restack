use std::path::{Path, PathBuf};

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{ci_service, provider_service};
use crate::error::Result;

#[derive(Subcommand)]
pub enum CiCommand {
    /// Show CI status for topics
    Status {
        /// Repo ID
        #[arg(long)]
        repo: String,
    },
    /// Generate CI workflow files
    Generate {
        /// Repo ID
        #[arg(long)]
        repo: String,
        /// Print to stdout instead of writing files
        #[arg(long)]
        stdout: bool,
        /// Output directory (default: current directory)
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
}

pub fn handle(conn: &Connection, cmd: &CiCommand, _cwd: &Path) -> Result<String> {
    match cmd {
        CiCommand::Status { repo } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let statuses = ci_service::get_ci_status(conn, &repo)?;
            Ok(serde_json::to_string_pretty(&statuses)?)
        }
        CiCommand::Generate {
            repo,
            stdout,
            output,
        } => {
            let repo = provider_service::load_repo(conn, repo)?;
            let files = ci_service::generate_workflow(
                repo.provider,
                output.as_deref(),
                *stdout,
            )?;
            Ok(serde_json::to_string_pretty(&files)?)
        }
    }
}
