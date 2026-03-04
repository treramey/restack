use std::path::Path;

use rusqlite::Connection;

use crate::db::{env_repo, topic_env_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::git;
use crate::id::RepoId;
use crate::types::{
    ChangelogEntry, ChangelogSection, ConventionalCommit, ReleaseInfo, Topic, TopicStatus,
};
use crate::version::{BumpType, SemVer};

use super::rebuild_service;

/// Preview a release: determine version bump, generate changelog. Does NOT create a tag.
pub fn prepare(
    _conn: &Connection,
    _repo_id: &RepoId,
    repo_path: &Path,
    bump_override: Option<BumpType>,
) -> Result<ReleaseInfo> {
    git::fetch(repo_path)?;

    let latest_tag = git::describe_latest_tag(repo_path)?;

    let current_version = match &latest_tag {
        Some(tag) => SemVer::parse(tag)?,
        None => SemVer {
            major: 0,
            minor: 0,
            patch: 0,
        },
    };

    // Get commits since last tag with sha|||subject format
    let commits =
        git::log_since(repo_path, latest_tag.as_deref(), "%H|||%s")?;
    let conventional = git::parse_conventional_commits(&commits);

    let auto_bump = determine_bump_type(&conventional);
    let bump_type = bump_override.unwrap_or(auto_bump);
    let new_version = current_version.bump(bump_type);

    // Get merge commit subjects for topic extraction
    let merge_subjects: Vec<String> = git::log_since(
        repo_path,
        latest_tag.as_deref(),
        "%s",
    )?
    .into_iter()
    .filter(|s| s.starts_with("Merge branch '"))
    .collect();
    let merged_topics = git::extract_topics_from_merges(&merge_subjects);

    let changelog = build_changelog(&conventional, &merged_topics);

    Ok(ReleaseInfo {
        version: new_version.to_string(),
        tag: new_version.to_tag(),
        bump_type,
        changelog,
        previous_version: latest_tag.map(|t| {
            t.strip_prefix('v')
                .unwrap_or(&t)
                .to_string()
        }),
    })
}

/// Cut a release: merge maint, tag, preserve maint branches, push, graduate topics.
pub fn cut(
    conn: &Connection,
    repo_id: &RepoId,
    repo_path: &Path,
    release_info: &ReleaseInfo,
    dry_run: bool,
) -> Result<()> {
    // Merge maint into master if needed
    merge_maint_to_master(repo_path)?;

    // Create annotated tag on master
    let tag_msg = format!("Release {}", release_info.version);
    git::tag_create(repo_path, &release_info.tag, &tag_msg)?;

    // Determine if maint-X.Y preservation is needed
    if let Some(ref prev) = release_info.previous_version {
        let prev_ver = SemVer::parse(prev)?;
        let new_ver = SemVer::parse(&release_info.version)?;

        let old_xy = format!("{}.{}", prev_ver.major, prev_ver.minor);
        let new_xy = format!("{}.{}", new_ver.major, new_ver.minor);

        if old_xy != new_xy {
            // Minor or major bump: preserve current maint as maint-X.Y
            let maint_branch = format!("maint-{old_xy}");
            let _ = git::branch_update_to(repo_path, &maint_branch, "origin/maint");
        }
    }

    // Update maint to point at master
    git::branch_update_to(repo_path, "maint", "master")?;

    if !dry_run {
        // Push master with tags
        git::push(repo_path, "master")?;
        git::push_tag(repo_path, &release_info.tag)?;
    }

    // Graduate topics from all environments
    graduate_topics(conn, repo_id, repo_path, dry_run)?;

    Ok(())
}

/// Merge maint into master using merge-tree plumbing (object-level, no working-tree mutation).
///
/// Returns `true` if a merge was performed, `false` if maint had nothing to merge.
pub fn merge_maint_to_master(repo_path: &Path) -> Result<bool> {
    let has_commits =
        git::has_commits_between(repo_path, "origin/master", "origin/maint")?;

    if !has_commits {
        return Ok(false);
    }

    match git::merge_tree(repo_path, "origin/master", "origin/maint")? {
        git::MergeTreeResult::Success { tree_oid } => {
            let master_sha = git::resolve_ref(repo_path, "origin/master")?;
            let maint_sha = git::resolve_ref(repo_path, "origin/maint")?;
            let merge_sha = git::commit_tree(
                repo_path,
                &tree_oid,
                &[&master_sha, &maint_sha],
                "Merge branch 'maint' into master",
            )?;
            git::update_ref(repo_path, "master", &merge_sha)?;
            Ok(true)
        }
        git::MergeTreeResult::Conflict { info } => Err(RestackError::MaintMergeConflict {
            reason: format!(
                "Cannot auto-merge maint into master. Resolve conflicts manually.\n{info}"
            ),
        }),
    }
}

/// Graduate topics that have been merged into master.
///
/// For each environment (ordered by ordinal), checks every topic to see if its
/// branch is an ancestor of `origin/master`. Graduated topics are removed from
/// their environment and marked as `TopicStatus::Graduated`.
///
/// After graduation, all environments are rebuilt to reflect the changes.
pub fn graduate_topics(
    conn: &Connection,
    repo_id: &RepoId,
    repo_path: &Path,
    dry_run: bool,
) -> Result<Vec<Topic>> {
    let envs = env_repo::list_envs(conn, Some(repo_id))?;
    let mut graduated = Vec::new();

    for env in &envs {
        let topics = topic_env_repo::get_topics_in_env(conn, &env.id)?;

        for topic in topics {
            if let Ok(true) = git::is_ancestor(repo_path, &topic.branch, "origin/master") {
                // Topic is merged into master — graduate it
                topic_env_repo::remove_topic_from_env(conn, &topic.id, &env.id)?;
                topic_repo::update_topic_status(conn, &topic.id, TopicStatus::Graduated)?;
                graduated.push(topic);
            }
        }
    }

    // Rebuild all environments after graduation
    rebuild_service::rebuild_all(conn, repo_id, repo_path, dry_run, false)?;

    Ok(graduated)
}

/// Determine the bump type from conventional commits.
/// Major if any breaking, minor if any feat, patch otherwise.
fn determine_bump_type(commits: &[ConventionalCommit]) -> BumpType {
    if commits.iter().any(|c| c.breaking) {
        BumpType::Major
    } else if commits.iter().any(|c| c.commit_type == "feat") {
        BumpType::Minor
    } else {
        BumpType::Patch
    }
}

/// Build changelog sections from parsed conventional commits and merged topics.
fn build_changelog(
    commits: &[ConventionalCommit],
    merged_topics: &[String],
) -> Vec<ChangelogSection> {
    let mut sections = Vec::new();

    // Features
    let features: Vec<ChangelogEntry> = commits
        .iter()
        .filter(|c| c.commit_type == "feat")
        .map(|c| ChangelogEntry {
            description: c.description.clone(),
            sha: c.sha.clone(),
            scope: c.scope.clone(),
        })
        .collect();

    if !features.is_empty() {
        sections.push(ChangelogSection {
            title: "Features".to_string(),
            entries: features,
        });
    }

    // Bug Fixes
    let fixes: Vec<ChangelogEntry> = commits
        .iter()
        .filter(|c| c.commit_type == "fix")
        .map(|c| ChangelogEntry {
            description: c.description.clone(),
            sha: c.sha.clone(),
            scope: c.scope.clone(),
        })
        .collect();

    if !fixes.is_empty() {
        sections.push(ChangelogSection {
            title: "Bug Fixes".to_string(),
            entries: fixes,
        });
    }

    // Merged Topics
    if !merged_topics.is_empty() {
        let topic_entries: Vec<ChangelogEntry> = merged_topics
            .iter()
            .map(|t| ChangelogEntry {
                description: t.clone(),
                sha: String::new(),
                scope: None,
            })
            .collect();

        sections.push(ChangelogSection {
            title: "Merged Topics".to_string(),
            entries: topic_entries,
        });
    }

    sections
}
