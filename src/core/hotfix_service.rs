use std::path::Path;

use rusqlite::Connection;

use crate::error::{RestackError, Result};
use crate::git;
use crate::id::RepoId;
use crate::types::HotfixInfo;
use crate::version::{BumpType, SemVer};

/// Create a hotfix branch from a maint branch.
///
/// Fetches latest, then creates `hotfix/{next_patch_version}` from `origin/{maint_branch}`.
/// Returns the new branch name.
pub fn create_hotfix(
    _conn: &Connection,
    _repo_id: &RepoId,
    repo_path: &Path,
    maint_branch: &str,
) -> Result<String> {
    git::fetch(repo_path)?;

    // Verify maint branch exists
    let maint_ref = format!("origin/{maint_branch}");
    git::resolve_ref(repo_path, &maint_ref)?;

    // Determine next patch version from latest tag
    let latest_tag = git::describe_latest_tag(repo_path)?;
    let current = match &latest_tag {
        Some(tag) => SemVer::parse(tag)?,
        None => SemVer {
            major: 0,
            minor: 0,
            patch: 0,
        },
    };
    let next = current.bump(BumpType::Patch);
    let branch_name = format!("hotfix/{next}");

    // Create the branch from maint
    git::branch_create(repo_path, &branch_name, &maint_ref)?;

    Ok(branch_name)
}

/// Release a hotfix: patch bump, tag, push, optionally merge to master.
///
/// Only `maint` merges to master. `maint-X.Y` branches are older release lines
/// and do NOT merge to master (per the hotfix workflow convention).
pub fn release_hotfix(
    _conn: &Connection,
    _repo_id: &RepoId,
    repo_path: &Path,
    maint_branch: &str,
    dry_run: bool,
) -> Result<HotfixInfo> {
    git::fetch(repo_path)?;

    // Get latest tag on the maint branch
    let latest_tag = git::describe_latest_tag(repo_path)?;
    let current = match &latest_tag {
        Some(tag) => SemVer::parse(tag)?,
        None => SemVer {
            major: 0,
            minor: 0,
            patch: 0,
        },
    };

    let new_version = current.bump(BumpType::Patch);
    let tag = new_version.to_tag();
    let tag_msg = format!("Hotfix {new_version}");

    // Create annotated tag on maint branch
    git::tag_create(repo_path, &tag, &tag_msg)?;

    let mut merged_to_master = false;

    if !dry_run {
        // Push maint branch with tags
        git::push(repo_path, maint_branch)?;
        git::push_tag(repo_path, &tag)?;

        // Only merge maint -> master, not maint-X.Y
        if maint_branch == "maint" {
            merged_to_master = merge_hotfix_to_master(repo_path, maint_branch)?;
        }
    }

    Ok(HotfixInfo {
        version: new_version.to_string(),
        tag,
        maint_branch: maint_branch.to_string(),
        merged_to_master,
    })
}

/// Merge hotfix from maint into master using merge-tree plumbing.
///
/// Returns `true` if merge was performed, `false` if nothing to merge.
/// Only called for `maint`, never for `maint-X.Y`.
fn merge_hotfix_to_master(repo_path: &Path, maint_branch: &str) -> Result<bool> {
    let maint_ref = format!("origin/{maint_branch}");

    let has_commits = git::has_commits_between(repo_path, "origin/master", &maint_ref)?;
    if !has_commits {
        return Ok(false);
    }

    match git::merge_tree(repo_path, "origin/master", &maint_ref)? {
        git::MergeTreeResult::Success { tree_oid } => {
            let master_sha = git::resolve_ref(repo_path, "origin/master")?;
            let maint_sha = git::resolve_ref(repo_path, &maint_ref)?;
            let msg = format!("Merge branch '{maint_branch}' into master (hotfix)");
            let merge_sha =
                git::commit_tree(repo_path, &tree_oid, &[&master_sha, &maint_sha], &msg)?;
            git::update_ref(repo_path, "master", &merge_sha)?;
            git::push(repo_path, "master")?;
            Ok(true)
        }
        git::MergeTreeResult::Conflict { info } => Err(RestackError::MaintMergeConflict {
            reason: format!(
                "Cannot auto-merge {maint_branch} into master. Resolve manually:\n\
                 git checkout master && git merge origin/{maint_branch}\n{info}"
            ),
        }),
    }
}
