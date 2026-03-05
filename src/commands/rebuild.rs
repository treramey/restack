use std::path::{Path, PathBuf};

use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::{promote_service, rebuild_service, repo_service};
use crate::db::{env_repo, repo_repo};
use crate::error::Result;
use crate::id::{EnvId, RepoId};
use crate::types::Rebuild;

#[derive(Subcommand)]
pub enum RebuildCommand {
    /// List all rebuilds
    List,
    /// Rebuild a specific environment
    Env {
        /// Environment ID
        env: String,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
        /// Interactively resolve conflicts instead of auto-skipping
        #[arg(long, short)]
        interactive: bool,
    },
    /// Rebuild all environments for a repo (or all repos with --all-repos)
    All {
        /// Repo ID (required unless --all-repos is set)
        repo: Option<String>,
        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
        /// Rebuild all environments across all tracked repos
        #[arg(long)]
        all_repos: bool,
        /// Interactively resolve conflicts instead of auto-skipping
        #[arg(long, short)]
        interactive: bool,
    },
    /// Watch mode: periodically auto-promote and rebuild changed environments
    Watch {
        /// Poll interval in seconds
        #[arg(long, default_value = "60")]
        interval: u64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiRepoRebuildResult {
    repo_name: String,
    repo_id: RepoId,
    rebuilds: Vec<Rebuild>,
}

pub fn handle(conn: &Connection, cmd: &RebuildCommand, repo_path: &Path) -> Result<String> {
    match cmd {
        RebuildCommand::List => {
            let rebuilds = crate::db::rebuild_repo::list_rebuilds(conn)?;
            Ok(serde_json::to_string_pretty(&rebuilds)?)
        }
        RebuildCommand::Env {
            env,
            dry_run,
            interactive,
        } => {
            let env_id: EnvId = env
                .parse()
                .map_err(|_| crate::error::RestackError::InvalidId(env.clone()))?;
            // Look up env and repo from DB to get the correct repo path
            let env_rec = env_repo::get_env(conn, &env_id)?;
            let repo = repo_repo::get_repo(conn, &env_rec.repo_id)?;
            let repo_path = PathBuf::from(&repo.path);
            let rebuild =
                rebuild_service::rebuild_env(conn, &env_id, &repo_path, *dry_run, *interactive)?;
            Ok(serde_json::to_string_pretty(&rebuild)?)
        }
        RebuildCommand::All {
            repo,
            dry_run,
            all_repos,
            interactive,
        } => {
            if *all_repos {
                let repos = repo_service::list_repos(conn)?;
                let mut results = Vec::new();

                for r in &repos {
                    let path = PathBuf::from(&r.path);
                    let rebuilds =
                        rebuild_service::rebuild_all(conn, &r.id, &path, *dry_run, *interactive)?;
                    results.push(MultiRepoRebuildResult {
                        repo_name: r.name.clone(),
                        repo_id: r.id.clone(),
                        rebuilds,
                    });
                }

                Ok(serde_json::to_string_pretty(&results)?)
            } else {
                let repo_str = repo.as_deref().ok_or_else(|| {
                    crate::error::RestackError::InvalidId("(none provided)".to_string())
                })?;
                let repo_id: RepoId = repo_str
                    .parse()
                    .map_err(|_| crate::error::RestackError::InvalidId(repo_str.to_string()))?;
                let rebuilds = rebuild_service::rebuild_all(
                    conn,
                    &repo_id,
                    repo_path,
                    *dry_run,
                    *interactive,
                )?;
                Ok(serde_json::to_string_pretty(&rebuilds)?)
            }
        }
        RebuildCommand::Watch { interval } => handle_watch(conn, repo_path, *interval),
    }
}

fn handle_watch(conn: &Connection, _repo_path: &Path, interval_secs: u64) -> Result<String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let _ = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    });

    eprintln!(
        "Restack watch mode started (interval: {}s). Press Ctrl+C to stop.",
        interval_secs
    );

    let mut cycles = 0u64;
    let mut total_promoted = 0u64;

    while running.load(Ordering::SeqCst) {
        cycles += 1;
        let tick_start = chrono::Utc::now();
        eprintln!(
            "[{}] Running auto-promote cycle...",
            tick_start.format("%H:%M:%S")
        );

        match promote_service::promote_auto(conn) {
            Ok(result) => {
                if result.promoted.is_empty() {
                    eprintln!(
                        "  No topics to auto-promote ({} CI statuses refreshed)",
                        result.refreshed_topics
                    );
                } else {
                    total_promoted += result.promoted.len() as u64;
                    eprintln!(
                        "  Auto-promoted {} topic(s) into env(s): {}",
                        result.promoted.len(),
                        result.envs_changed.join(", ")
                    );
                }
            }
            Err(e) => {
                eprintln!("  Warning: auto-promote failed: {e}");
            }
        }

        // Sleep in small increments to check the stop flag
        let mut remaining = interval_secs;
        while remaining > 0 && running.load(Ordering::SeqCst) {
            let sleep_for = remaining.min(2);
            thread::sleep(Duration::from_secs(sleep_for));
            remaining -= sleep_for;
        }
    }

    eprintln!("\nWatch mode stopped.");
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "cycles": cycles,
        "totalPromoted": total_promoted,
        "stopped": "graceful"
    }))?)
}
