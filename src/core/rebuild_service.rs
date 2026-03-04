use std::path::Path;

use dialoguer::Select;
use rusqlite::Connection;

use crate::db::{conflict_repo, env_repo, rebuild_repo, repo_repo, topic_env_repo};
use crate::error::Result;
use crate::git;
use crate::id::{EnvId, RepoId};
use crate::types::{Rebuild, RebuildStatus, Topic, TopicStatus};

/// Action chosen by the user during interactive conflict resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConflictAction {
    Skip,
    OpenEditor,
    Retry,
    Abort,
}

/// Outcome of attempting to merge a single topic.
enum MergeOutcome {
    Merged,
    Skipped,
    Abort,
}

/// Prompt the user to choose how to handle a merge conflict.
fn prompt_conflict_action(topic: &Topic, conflict_info: &str) -> ConflictAction {
    eprintln!("\n╭─ Conflict detected: topic '{}'", topic.branch);
    eprintln!("│");
    for line in conflict_info.lines().take(20) {
        eprintln!("│  {line}");
    }
    eprintln!("╰─");

    let items = &[
        "Skip this topic (remove from environment)",
        "Open in $EDITOR to view conflicts",
        "Retry merge (after manual resolution)",
        "Abort rebuild",
    ];

    let selection = Select::new()
        .with_prompt("How would you like to handle this conflict?")
        .items(items)
        .default(0)
        .interact();

    match selection {
        Ok(0) => ConflictAction::Skip,
        Ok(1) => ConflictAction::OpenEditor,
        Ok(2) => ConflictAction::Retry,
        Ok(3) => ConflictAction::Abort,
        _ => ConflictAction::Skip,
    }
}

/// Open conflict info in the user's $EDITOR for review.
fn open_in_editor(conflict_info: &str) {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let unique_id = std::process::id();
    let tmp_path = std::env::temp_dir().join(format!("restack-conflict-{unique_id}.txt"));

    if std::fs::write(&tmp_path, conflict_info).is_ok() {
        let _ = std::process::Command::new(&editor)
            .arg(&tmp_path)
            .status();
    } else {
        eprintln!("Failed to write conflict info to temp file");
    }
}

/// Rebuild a single environment using object-level merges (`git merge-tree`).
///
/// Never touches the working tree or index — all merge operations happen at
/// the git object level. Safe to run while the user has uncommitted work.
///
/// For "dev" environments (ordinal > 0 where a lower-ordinal env exists):
///   Phase 1: merge topics that are ALSO in the lower-ordinal env (e.g. staging)
///   Insert marker commit: "### Match '<lower_env_name>'"
///   Phase 2: merge topics that are ONLY in this env
///
/// For staging or other environments: single-phase merge of all topics.
pub fn rebuild_env(
    conn: &Connection,
    env_id: &EnvId,
    repo_path: &Path,
    dry_run: bool,
    interactive: bool,
) -> Result<Rebuild> {
    let env = env_repo::get_env(conn, env_id)?;
    let repo = repo_repo::get_repo(conn, &env.repo_id)?;
    let topics = topic_env_repo::get_topics_in_env(conn, env_id)?;

    let rebuild = rebuild_repo::create_rebuild(conn, env_id)?;

    let mut merged_count: i32 = 0;
    let mut conflicted_count: i32 = 0;
    let mut aborted = false;

    // Fetch latest refs
    git::fetch(repo_path)?;

    // Resolve base to a commit SHA — all subsequent work is object-level
    let base_ref = format!("origin/{}", repo.base_branch);
    let mut current_sha = git::resolve_ref(repo_path, &base_ref)?;

    // Determine if this is a two-phase rebuild (dev-style)
    let lower_env = find_lower_ordinal_env(conn, &env.repo_id, env.ordinal)?;

    if let Some(ref lower) = lower_env {
        // TWO-PHASE REBUILD
        let lower_topics = topic_env_repo::get_topics_in_env(conn, &lower.id)?;
        let lower_branches: Vec<&str> = lower_topics.iter().map(|t| t.branch.as_str()).collect();

        let (phase1, phase2): (Vec<&Topic>, Vec<&Topic>) = topics
            .iter()
            .partition(|t| lower_branches.contains(&t.branch.as_str()));

        // Phase 1: topics in BOTH this env and the lower env
        for topic in &phase1 {
            match merge_topic_interactive(
                repo_path, topic, &mut current_sha, conn, &rebuild,
                &mut conflicted_count, interactive,
            )? {
                MergeOutcome::Merged => merged_count += 1,
                MergeOutcome::Skipped => {}
                MergeOutcome::Abort => { aborted = true; break; }
            }
        }

        if !aborted {
            // Marker commit (same tree, new commit — no working-tree mutation)
            let tree_oid = git::rev_parse_tree(repo_path, &current_sha)?;
            let marker_msg = format!("### Match '{}'", lower.name);
            current_sha =
                git::commit_tree(repo_path, &tree_oid, &[&current_sha], &marker_msg)?;

            // Phase 2: topics ONLY in this env
            for topic in &phase2 {
                match merge_topic_interactive(
                    repo_path, topic, &mut current_sha, conn, &rebuild,
                    &mut conflicted_count, interactive,
                )? {
                    MergeOutcome::Merged => merged_count += 1,
                    MergeOutcome::Skipped => {}
                    MergeOutcome::Abort => { aborted = true; break; }
                }
            }
        }
    } else {
        // SINGLE-PHASE REBUILD
        for topic in &topics {
            match merge_topic_interactive(
                repo_path, topic, &mut current_sha, conn, &rebuild,
                &mut conflicted_count, interactive,
            )? {
                MergeOutcome::Merged => merged_count += 1,
                MergeOutcome::Skipped => {}
                MergeOutcome::Abort => { aborted = true; break; }
            }
        }
    }

    // Point env branch at the final commit and push — only when not dry_run and not aborted
    if !dry_run && !aborted {
        git::update_ref(repo_path, &env.branch, &current_sha)?;
        git::force_push(repo_path, &env.branch)?;
    }

    let status = if aborted {
        RebuildStatus::Failed
    } else if conflicted_count > 0 && merged_count > 0 {
        RebuildStatus::Partial
    } else if conflicted_count > 0 {
        RebuildStatus::Failed
    } else {
        RebuildStatus::Success
    };

    rebuild_repo::complete_rebuild(
        conn,
        &rebuild.id,
        status,
        merged_count,
        conflicted_count,
        Some(&current_sha),
    )?;

    // Best-effort conflict notification via provider
    if conflicted_count > 0 {
        let conflicts = conflict_repo::list_conflicts(conn, &rebuild.id).unwrap_or_default();
        if let Err(e) = crate::core::provider_service::notify_conflicts(conn, &repo, &conflicts) {
            eprintln!("Warning: failed to notify conflicts: {e}");
        }
    }

    Ok(Rebuild {
        id: rebuild.id,
        env_id: env.id,
        started_at: rebuild.started_at,
        completed_at: Some(chrono::Utc::now()),
        status,
        topics_merged: merged_count,
        topics_conflicted: conflicted_count,
        result_sha: Some(current_sha),
    })
}

/// Rebuild all environments for a repo in ordinal order.
/// Staging (lower ordinal) must be rebuilt first so dev phase1 has current data.
pub fn rebuild_all(
    conn: &Connection,
    repo_id: &RepoId,
    repo_path: &Path,
    dry_run: bool,
    interactive: bool,
) -> Result<Vec<Rebuild>> {
    let envs = env_repo::list_envs(conn, Some(repo_id))?;
    let mut results = Vec::new();

    for env in envs {
        let rebuild = rebuild_env(conn, &env.id, repo_path, dry_run, interactive)?;
        results.push(rebuild);
    }

    Ok(results)
}

/// Merge a single topic with optional interactive conflict handling.
///
/// In non-interactive mode, conflicts are auto-skipped (existing behavior).
/// In interactive mode, the user is prompted to skip, view in editor, retry, or abort.
fn merge_topic_interactive(
    repo_path: &Path,
    topic: &Topic,
    current_sha: &mut String,
    conn: &Connection,
    rebuild: &Rebuild,
    conflicted_count: &mut i32,
    interactive: bool,
) -> Result<MergeOutcome> {
    // Skip if topic is already an ancestor of current
    if let Ok(true) = git::is_ancestor(repo_path, &topic.branch, current_sha) {
        return Ok(MergeOutcome::Skipped);
    }

    loop {
        match git::merge_tree(repo_path, current_sha, &topic.branch)? {
            git::MergeTreeResult::Success { tree_oid } => {
                let topic_sha = git::resolve_ref(repo_path, &topic.branch)?;
                let msg = format!("Merge branch '{}'", topic.branch);
                let merge_sha =
                    git::commit_tree(repo_path, &tree_oid, &[current_sha, &topic_sha], &msg)?;
                *current_sha = merge_sha;
                return Ok(MergeOutcome::Merged);
            }
            git::MergeTreeResult::Conflict { info } => {
                if !interactive {
                    *conflicted_count += 1;
                    conflict_repo::create_conflict(conn, &rebuild.id, &topic.id, None)?;
                    crate::db::topic_repo::update_topic_status(
                        conn, &topic.id, TopicStatus::Conflict,
                    )?;
                    let _ = topic_env_repo::remove_topic_from_env(
                        conn, &topic.id, &rebuild.env_id,
                    );
                    return Ok(MergeOutcome::Skipped);
                }

                match prompt_conflict_action(topic, &info) {
                    ConflictAction::Skip => {
                        *conflicted_count += 1;
                        conflict_repo::create_conflict(conn, &rebuild.id, &topic.id, None)?;
                        crate::db::topic_repo::update_topic_status(
                            conn, &topic.id, TopicStatus::Conflict,
                        )?;
                        let _ = topic_env_repo::remove_topic_from_env(
                            conn, &topic.id, &rebuild.env_id,
                        );
                        return Ok(MergeOutcome::Skipped);
                    }
                    ConflictAction::OpenEditor => {
                        open_in_editor(&info);
                        // Loop continues — user will be re-prompted
                    }
                    ConflictAction::Retry => {
                        let _ = git::fetch(repo_path);
                        // Loop retries the merge
                    }
                    ConflictAction::Abort => {
                        eprintln!("Rebuild aborted by user.");
                        return Ok(MergeOutcome::Abort);
                    }
                }
            }
        }
    }
}

/// Find the environment with the next lower ordinal for two-phase rebuild.
fn find_lower_ordinal_env(
    conn: &Connection,
    repo_id: &RepoId,
    current_ordinal: i32,
) -> Result<Option<crate::types::Environment>> {
    let envs = env_repo::list_envs(conn, Some(repo_id))?;
    Ok(envs
        .into_iter()
        .filter(|e| e.ordinal < current_ordinal)
        .max_by_key(|e| e.ordinal))
}
