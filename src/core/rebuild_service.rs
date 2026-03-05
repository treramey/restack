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
        let _ = std::process::Command::new(&editor).arg(&tmp_path).status();
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
    _repo_path: &Path,
    dry_run: bool,
    interactive: bool,
) -> Result<Rebuild> {
    let env = env_repo::get_env(conn, env_id)?;
    let repo = repo_repo::get_repo(conn, &env.repo_id)?;
    let topics = topic_env_repo::get_topics_in_env(conn, env_id)?;

    let repo_path = Path::new(&repo.path);

    let rebuild = rebuild_repo::create_rebuild(conn, env_id)?;

    let mut merged_count: i32 = 0;
    let mut conflicted_count: i32 = 0;
    let mut aborted = false;

    git::fetch(repo_path)?;

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
                repo_path,
                topic,
                &mut current_sha,
                conn,
                &rebuild,
                &mut conflicted_count,
                interactive,
            )? {
                MergeOutcome::Merged => merged_count += 1,
                MergeOutcome::Skipped => {}
                MergeOutcome::Abort => {
                    aborted = true;
                    break;
                }
            }
        }

        if !aborted {
            // Marker commit (same tree, new commit — no working-tree mutation)
            let tree_oid = git::rev_parse_tree(repo_path, &current_sha)?;
            let marker_msg = format!("### Match '{}'", lower.name);
            current_sha = git::commit_tree(repo_path, &tree_oid, &[&current_sha], &marker_msg)?;

            // Phase 2: topics ONLY in this env
            for topic in &phase2 {
                match merge_topic_interactive(
                    repo_path,
                    topic,
                    &mut current_sha,
                    conn,
                    &rebuild,
                    &mut conflicted_count,
                    interactive,
                )? {
                    MergeOutcome::Merged => merged_count += 1,
                    MergeOutcome::Skipped => {}
                    MergeOutcome::Abort => {
                        aborted = true;
                        break;
                    }
                }
            }
        }
    } else {
        // SINGLE-PHASE REBUILD
        for topic in &topics {
            match merge_topic_interactive(
                repo_path,
                topic,
                &mut current_sha,
                conn,
                &rebuild,
                &mut conflicted_count,
                interactive,
            )? {
                MergeOutcome::Merged => merged_count += 1,
                MergeOutcome::Skipped => {}
                MergeOutcome::Abort => {
                    aborted = true;
                    break;
                }
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
                        conn,
                        &topic.id,
                        TopicStatus::Conflict,
                    )?;
                    let _ = topic_env_repo::remove_topic_from_env(conn, &topic.id, &rebuild.env_id);
                    return Ok(MergeOutcome::Skipped);
                }

                match prompt_conflict_action(topic, &info) {
                    ConflictAction::Skip => {
                        *conflicted_count += 1;
                        conflict_repo::create_conflict(conn, &rebuild.id, &topic.id, None)?;
                        crate::db::topic_repo::update_topic_status(
                            conn,
                            &topic.id,
                            TopicStatus::Conflict,
                        )?;
                        let _ =
                            topic_env_repo::remove_topic_from_env(conn, &topic.id, &rebuild.env_id);
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

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::db::schema::open_db;
    use crate::id::EnvId;

    /// Verify rebuild_env uses DB repo path, not the passed-in path.
    /// This tests the fix that made rebuild look up repo from DB.
    #[test]
    fn test_rebuild_from_outside_repo() {
        // 1. Create temp workspace with git repo
        let workspace = tempfile::tempdir().expect("create workspace temp dir");
        let repo_path = workspace.path().join("my-repo");
        std::fs::create_dir_all(&repo_path).expect("create repo dir");

        // 2. Init git repo with required branches
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .status()
            .expect("git init");

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .status()
            .expect("git config email");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .status()
            .expect("git config name");

        // Create initial commit on main
        std::fs::write(repo_path.join("README.md"), "# test\n").expect("write readme");
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .status()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&repo_path)
            .status()
            .expect("git commit");

        // Create dev and staging branches
        Command::new("git")
            .args(["branch", "dev"])
            .current_dir(&repo_path)
            .status()
            .expect("create dev branch");
        Command::new("git")
            .args(["branch", "staging"])
            .current_dir(&repo_path)
            .status()
            .expect("create staging branch");

        // 3. Add origin remote pointing to itself (required for fetch)
        Command::new("git")
            .args(["remote", "add", "origin", "."])
            .current_dir(&repo_path)
            .status()
            .expect("add origin remote");
        Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(&repo_path)
            .status()
            .expect("fetch origin");

        // 4. Create DB in workspace
        let db_path = workspace.path().join(".restack").join("workspace.db");
        let conn = open_db(&db_path).expect("open db");

        // 5. Insert repo record
        let repo_id = crate::id::RepoId::new();
        conn.execute(
            "INSERT INTO repos (id, name, path, provider, base_branch, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                &repo_id,
                "my-repo",
                repo_path.to_str().unwrap(),
                "unknown",
                "main",
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .expect("insert repo");

        // 6. Insert env record
        let env_id = EnvId::new();
        conn.execute(
            "INSERT INTO environments (id, repo_id, name, branch, ordinal) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![&env_id, &repo_id, "staging", "staging", 0],
        )
        .expect("insert env");

        // 7. Change to a different directory (outside the repo)
        let outside_dir = tempfile::tempdir().expect("create outside temp dir");

        // 8. Call rebuild_env with a DUMMY path - it should use DB path instead
        // dry_run=true to avoid any push operations
        let result = rebuild_env(
            &conn,
            &env_id,
            Path::new("/dummy/path/that/does/not/exist"),
            true,  // dry_run
            false, // interactive
        );

        // 9. Assert success - the function should NOT fail with "not a git repository"
        // because it uses the DB repo path, not the dummy path
        assert!(
            result.is_ok(),
            "rebuild_env should succeed using DB path, got error: {:?}",
            result.err()
        );

        let rebuild = result.unwrap();
        assert_eq!(rebuild.status, crate::types::RebuildStatus::Success);
        assert_eq!(rebuild.topics_merged, 0); // no topics in env

        // Cleanup: change back to original dir (not strictly necessary)
        drop(outside_dir);
        drop(workspace);
    }
}
