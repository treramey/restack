use std::path::Path;

use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::core::{
    discovery_service, env_sync_service, promote_service, repo_service, topic_service,
};
use crate::db::{env_repo, repo_repo, topic_env_repo, topic_repo};
use crate::error::Result;
use crate::id::RepoId;
use crate::types::Topic;

#[derive(Subcommand)]
#[command(disable_help_subcommand = true)]
pub enum TopicCommand {
    /// Promote a topic to an integration branch (auto-promotes to next integration branch if not specified)
    Promote {
        /// Topic ID or branch name (omit when using --all)
        topic: Option<String>,
        /// Target integration branch name (auto-detects next integration branch if omitted; required when using --all)
        env: Option<String>,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
        /// Promote all topics in the specified integration branch (requires integration branch argument)
        #[arg(long)]
        all: bool,
    },
    /// Demote a topic from an integration branch (auto-demotes from current integration branch if not specified)
    Demote {
        /// Topic ID or branch name (omit when using --all)
        topic: Option<String>,
        /// Integration branch name to remove from (auto-detects current integration branch if omitted; required when using --all)
        env: Option<String>,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
        /// Demote all topics from the specified integration branch (requires integration branch argument)
        #[arg(long)]
        all: bool,
    },
    /// Close a topic: delete from origin and remove from all integration branches
    Close {
        /// Topic ID or branch name
        id: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// List tracked topics
    List {
        /// Filter by repo ID
        #[arg(long)]
        repo: Option<String>,
        /// List topics across all tracked repos
        #[arg(long)]
        all_repos: bool,
    },
    /// Show topic status
    Status {
        /// Topic ID or branch name
        id: String,
        /// Repo ID or name (auto-detected if not specified)
        #[arg(long)]
        repo: Option<String>,
    },
    /// List all topic-environment associations
    Envs,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiRepoTopics {
    repo_name: String,
    repo_id: RepoId,
    topics: Vec<Topic>,
}

pub fn handle(
    conn: &Connection,
    cmd: &TopicCommand,
    workspace_root: &Path,
    no_reconcile: bool,
) -> Result<String> {
    match cmd {
        TopicCommand::Close { id, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            let repo_path = std::path::Path::new(&repo.path);
            let topic = crate::db::topic_repo::get_topic_by_branch(conn, &repo.id, id)?
                .or_else(|| {
                    id.parse()
                        .ok()
                        .and_then(|tid| crate::db::topic_repo::get_topic(conn, &tid).ok())
                })
                .ok_or_else(|| {
                    crate::error::RestackError::TopicNotFound(id.parse().unwrap_or_default())
                })?;

            let _ = crate::git::branch_delete(repo_path, &topic.branch, true);

            // Remove from all environments and delete from DB
            crate::db::topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;
            crate::db::topic_repo::delete_topic(conn, &topic.id)?;

            Ok(serde_json::json!({ "deleted": true, "branch": topic.branch }).to_string())
        }
        TopicCommand::Promote {
            topic,
            env,
            repo,
            all,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            if !no_reconcile {
                let r_path = std::path::Path::new(&repo.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo.id, r_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }

            if *all {
                // When using --all, the env can be passed as the first positional argument
                // (since topic is optional). Check if topic looks like an env name when env is None.
                let source_env = env.as_deref().or_else(|| topic.as_deref());
                return promote_all_topics(conn, &repo, source_env, workspace_root, no_reconcile);
            }

            let topic = topic.as_ref().ok_or_else(|| {
                crate::error::RestackError::RepoConfigValidation(
                    "Must specify a topic or use --all".to_string(),
                )
            })?;

            let target_env = match env {
                Some(e) => e.clone(),
                None => {
                    let all_envs = env_repo::list_envs(conn, Some(&repo.id))?;
                    let topic_obj = topic_repo::get_topic_by_branch(conn, &repo.id, topic)?
                        .ok_or_else(|| {
                            crate::error::RestackError::TopicNotFound(
                                topic.parse().unwrap_or_default(),
                            )
                        })?;
                    let assigned_env_ids = topic_env_repo::get_envs_for_topic(conn, &topic_obj.id)?;

                    let current_lowest_ordinal = assigned_env_ids
                        .iter()
                        .filter_map(|env_id| all_envs.iter().find(|e| &e.id == env_id))
                        .map(|e| e.ordinal)
                        .min();

                    let next_env = match current_lowest_ordinal {
                        Some(ord) => all_envs
                            .iter()
                            .filter(|e| e.ordinal < ord)
                            .max_by_key(|e| e.ordinal),
                        None => all_envs.iter().max_by_key(|e| e.ordinal),
                    };

                    match next_env {
                        Some(e) => e.name.clone(),
                        None => return Err(crate::error::RestackError::RepoConfigValidation(
                            "No next integration branch available - topic may already be in highest integration branch".to_string()
                        )),
                    }
                }
            };

            let repo_path = std::path::Path::new(&repo.path);
            let result =
                promote_service::promote_to(conn, topic, &target_env, &repo.id, repo_path, false)?;

            let all_envs = env_repo::list_envs(conn, Some(&repo.id))?;
            let is_highest_env = all_envs
                .iter()
                .filter(|e| e.name == target_env)
                .all(|e| all_envs.iter().all(|other| other.ordinal >= e.ordinal));

            if is_highest_env && !all_envs.is_empty() {
                eprintln!(
                    "\n✅ Topic '{}' promoted to '{}' (highest integration branch)",
                    result.topic.branch, target_env
                );
                eprintln!("   Ready for production! Create a PR to merge into the base branch.");
            }

            Ok(serde_json::to_string_pretty(&result)?)
        }
        TopicCommand::Demote {
            topic,
            env,
            repo,
            all,
        } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            if !no_reconcile {
                let r_path = std::path::Path::new(&repo.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo.id, r_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }

            if *all {
                // When using --all, the env can be passed as the first positional argument
                // (since topic is optional). Use topic as env when env is None.
                let source_env = env.as_deref().or_else(|| topic.as_deref());
                return demote_all_topics(conn, &repo, source_env, workspace_root, no_reconcile);
            }

            let topic = topic.as_ref().ok_or_else(|| {
                crate::error::RestackError::RepoConfigValidation(
                    "Must specify a topic or use --all".to_string(),
                )
            })?;

            let target_env = match env {
                Some(e) => e.clone(),
                None => {
                    let all_envs = env_repo::list_envs(conn, Some(&repo.id))?;
                    let topic_obj = topic_repo::get_topic_by_branch(conn, &repo.id, topic)?
                        .ok_or_else(|| {
                            crate::error::RestackError::TopicNotFound(
                                topic.parse().unwrap_or_default(),
                            )
                        })?;
                    let assigned_env_ids = topic_env_repo::get_envs_for_topic(conn, &topic_obj.id)?;

                    let current_env = assigned_env_ids
                        .iter()
                        .filter_map(|env_id| all_envs.iter().find(|e| &e.id == env_id))
                        .min_by_key(|e| e.ordinal);

                    match current_env {
                        Some(e) => e.name.clone(),
                        None => {
                            return Err(crate::error::RestackError::RepoConfigValidation(
                                "Topic not assigned to any integration branch".to_string(),
                            ))
                        }
                    }
                }
            };

            let repo_path = std::path::Path::new(&repo.path);
            let result =
                promote_service::demote_from(conn, topic, &target_env, &repo.id, repo_path, false)?;

            let topic_obj =
                topic_repo::get_topic_by_branch(conn, &repo.id, topic)?.ok_or_else(|| {
                    crate::error::RestackError::TopicNotFound(topic.parse().unwrap_or_default())
                })?;
            let remaining_envs = topic_env_repo::get_envs_for_topic(conn, &topic_obj.id)?;

            if remaining_envs.is_empty() {
                eprintln!(
                    "\nℹ️  Topic '{}' is now unassigned from all integration branches",
                    result.topic.branch
                );
                eprintln!(
                    "   The branch still exists but is no longer tracked in any integration branch"
                );
            }

            Ok(serde_json::to_string_pretty(&result)?)
        }
        TopicCommand::List { repo, all_repos } => {
            // Auto-discover new branches before listing
            let config_path = workspace_root.join(".restack/config.toml");
            let cfg = if config_path.exists() {
                config::load_config(&config_path)?
            } else {
                config::default_config()
            };

            let repos_to_discover = if let Some(r) = repo {
                vec![repo_service::resolve_repo(conn, Some(r), workspace_root)?]
            } else {
                repo_repo::list_repos(conn).unwrap_or_default()
            };

            for r in &repos_to_discover {
                let repo_path = std::path::Path::new(&r.path);
                let _ = discovery_service::discover_topics(conn, &r.id, repo_path, &cfg);
            }

            if *all_repos {
                if !no_reconcile {
                    let repos = repo_repo::list_repos(conn)?;
                    for r in &repos {
                        let repo_path = std::path::Path::new(&r.path);
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
                    let topics = topic_service::list_topics(conn, Some(&r.id))?;
                    results.push(MultiRepoTopics {
                        repo_name: r.name.clone(),
                        repo_id: r.id.clone(),
                        topics,
                    });
                }
                Ok(serde_json::to_string_pretty(&results)?)
            } else {
                let repo_id = repo
                    .as_ref()
                    .map(|r| {
                        Ok::<_, crate::error::RestackError>(
                            repo_service::resolve_repo(conn, Some(r), workspace_root)?.id,
                        )
                    })
                    .transpose()?;
                if !no_reconcile {
                    if let Some(rid) = &repo_id {
                        if let Ok(r) = repo_repo::get_repo(conn, rid) {
                            let repo_path = std::path::Path::new(&r.path);
                            if let Some(summary) =
                                env_sync_service::maybe_reconcile_repo_envs(conn, &r.id, repo_path)?
                            {
                                eprintln!(
                                    "{}",
                                    env_sync_service::format_reconcile_summary(&summary)
                                );
                            }
                        }
                    }
                }
                let topics = topic_service::list_topics(conn, repo_id.as_ref())?;
                Ok(serde_json::to_string_pretty(&topics)?)
            }
        }
        TopicCommand::Envs => {
            let topic_envs = topic_env_repo::list_all_topic_environments(conn)?;
            Ok(serde_json::to_string_pretty(&topic_envs)?)
        }
        TopicCommand::Status { id, repo } => {
            let repo = repo_service::resolve_repo(conn, repo.as_deref(), workspace_root)?;
            if !no_reconcile {
                let repo_path = std::path::Path::new(&repo.path);
                if let Some(summary) =
                    env_sync_service::maybe_reconcile_repo_envs(conn, &repo.id, repo_path)?
                {
                    eprintln!("{}", env_sync_service::format_reconcile_summary(&summary));
                }
            }
            let status = topic_service::get_topic_status(conn, id, &repo.id)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
    }
}

use crate::types::Repo;

fn promote_all_topics(
    conn: &Connection,
    repo: &Repo,
    env: Option<&str>,
    _workspace_root: &Path,
    _no_reconcile: bool,
) -> Result<String> {
    let _all_envs = env_repo::list_envs(conn, Some(&repo.id))?;
    let source_env = match env {
        Some(e) => e.to_string(),
        None => {
            return Err(crate::error::RestackError::RepoConfigValidation(
                "Must specify an integration branch when using --all".to_string(),
            ))
        }
    };

    let env_obj = env_repo::get_env_by_name(conn, &repo.id, &source_env)?.ok_or_else(|| {
        crate::error::RestackError::EnvNotFound(source_env.parse().unwrap_or_default())
    })?;

    let topics = topic_env_repo::get_topics_in_env(conn, &env_obj.id)?;

    if topics.is_empty() {
        return Ok(serde_json::json!({
            "message": format!("No topics found in integration branch '{}'", source_env),
            "promoted": 0
        })
        .to_string());
    }

    let mut results = Vec::new();
    let repo_path = std::path::Path::new(&repo.path);

    for topic in &topics {
        match promote_service::promote_to(
            conn,
            &topic.branch,
            &source_env,
            &repo.id,
            repo_path,
            false,
        ) {
            Ok(result) => {
                results.push(serde_json::json!({
                    "topic": result.topic.branch,
                    "status": "promoted",
                    "to_env": result.env.name
                }));
            }
            Err(e) => {
                results.push(serde_json::json!({
                    "topic": topic.branch,
                    "status": "error",
                    "error": e.to_string()
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "source_env": source_env,
        "promoted": results.iter().filter(|r| r["status"] == "promoted").count(),
        "results": results
    })
    .to_string())
}

fn demote_all_topics(
    conn: &Connection,
    repo: &Repo,
    env: Option<&str>,
    _workspace_root: &Path,
    _no_reconcile: bool,
) -> Result<String> {
    let _all_envs = env_repo::list_envs(conn, Some(&repo.id))?;
    let source_env = match env {
        Some(e) => e.to_string(),
        None => {
            return Err(crate::error::RestackError::RepoConfigValidation(
                "Must specify an integration branch when using --all".to_string(),
            ))
        }
    };

    let env_obj = env_repo::get_env_by_name(conn, &repo.id, &source_env)?.ok_or_else(|| {
        crate::error::RestackError::EnvNotFound(source_env.parse().unwrap_or_default())
    })?;

    let topics = topic_env_repo::get_topics_in_env(conn, &env_obj.id)?;

    if topics.is_empty() {
        return Ok(serde_json::json!({
            "message": format!("No topics found in integration branch '{}'", source_env),
            "demoted": 0
        })
        .to_string());
    }

    let mut results = Vec::new();
    let repo_path = std::path::Path::new(&repo.path);

    for topic in &topics {
        match promote_service::demote_from(
            conn,
            &topic.branch,
            &source_env,
            &repo.id,
            repo_path,
            false,
        ) {
            Ok(result) => {
                results.push(serde_json::json!({
                    "topic": result.topic.branch,
                    "status": "demoted",
                    "from_env": result.env.name
                }));
            }
            Err(e) => {
                results.push(serde_json::json!({
                    "topic": topic.branch,
                    "status": "error",
                    "error": e.to_string()
                }));
            }
        }
    }

    let demoted_count = results.iter().filter(|r| r["status"] == "demoted").count();
    if demoted_count > 0 {
        eprintln!(
            "\nℹ️  {} topics demoted from '{}' and are now unassigned",
            demoted_count, source_env
        );
    }

    Ok(serde_json::json!({
        "source_env": source_env,
        "demoted": demoted_count,
        "results": results
    })
    .to_string())
}
