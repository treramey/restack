use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;

use crate::core::{hotfix_service, release_service};
use crate::error::{RestackError, Result};
use crate::version::BumpType;

#[derive(Subcommand)]
pub enum ReleaseCommand {
    /// Preview the next release (version bump, changelog)
    Prepare {
        /// Override bump type (major, minor, patch)
        #[arg(long)]
        bump: Option<String>,
    },
    /// Cut a release (tag, push, update maint, graduate topics)
    Cut {
        /// Override bump type
        #[arg(long)]
        bump: Option<String>,
    },
    /// Create a hotfix branch from maint
    Hotfix {
        /// Maint branch to hotfix from (default: maint)
        #[arg(long, default_value = "maint")]
        base: String,
    },
    /// Release a hotfix (patch bump, tag, push, merge to master)
    #[command(name = "hotfix-release")]
    HotfixRelease {
        /// Maint branch (default: maint)
        #[arg(long, default_value = "maint")]
        base: String,
    },
}

pub fn handle(
    conn: &Connection,
    cmd: &ReleaseCommand,
    repo_path: &Path,
    dry_run: bool,
) -> Result<String> {
    // Resolve repo from cwd
    let path_str = repo_path.to_string_lossy();
    let repo = crate::db::repo_repo::get_repo_by_path(conn, &path_str)?
        .ok_or_else(|| RestackError::NotAWorkspace)?;
    let repo_id = repo.id;

    match cmd {
        ReleaseCommand::Prepare { bump } => {
            let bump_type = bump
                .as_deref()
                .map(|b| b.parse::<BumpType>())
                .transpose()
                .map_err(|e| crate::error::RestackError::InvalidVersion { version: e })?;

            let info = release_service::prepare(conn, &repo_id, repo_path, bump_type)?;
            Ok(serde_json::to_string_pretty(&info)?)
        }
        ReleaseCommand::Cut { bump } => {
            let bump_type = bump
                .as_deref()
                .map(|b| b.parse::<BumpType>())
                .transpose()
                .map_err(|e| crate::error::RestackError::InvalidVersion { version: e })?;

            let info = release_service::prepare(conn, &repo_id, repo_path, bump_type)?;
            release_service::cut(conn, &repo_id, repo_path, &info, dry_run)?;
            Ok(serde_json::to_string_pretty(&info)?)
        }
        ReleaseCommand::Hotfix { base } => {
            let branch = hotfix_service::create_hotfix(conn, &repo_id, repo_path, base)?;
            let result = serde_json::json!({ "branch": branch });
            Ok(serde_json::to_string_pretty(&result)?)
        }
        ReleaseCommand::HotfixRelease { base } => {
            let info = hotfix_service::release_hotfix(conn, &repo_id, repo_path, base, dry_run)?;
            Ok(serde_json::to_string_pretty(&info)?)
        }
    }
}
