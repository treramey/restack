use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{repo_repo, topic_repo};
use crate::error::Result;
use crate::id::TopicId;
use crate::provider::{self, PrState};
use crate::types::{BranchOrigin, CiStatus, Conflict, Repo, TopicStatus};

// ---------------------------------------------------------------------------
// Return types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub created: i32,
    pub updated: i32,
    pub total_prs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiRefreshEntry {
    pub topic_id: TopicId,
    pub branch: String,
    pub old_status: Option<CiStatus>,
    pub new_status: CiStatus,
}

// ---------------------------------------------------------------------------
// Sync topics from pull requests
// ---------------------------------------------------------------------------

pub fn sync_topics_from_prs(conn: &Connection, repo: &Repo) -> Result<SyncResult> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let prs = adapter.list_prs(PrState::Open)?;

    let mut created = 0i32;
    let mut updated = 0i32;
    let total_prs = prs.len() as i32;

    for pr in &prs {
        match topic_repo::get_topic_by_branch(conn, &repo.id, &pr.head_branch)? {
            Some(topic) => {
                let pr_changed = topic.pr_id.as_deref() != Some(&pr.number)
                    || topic.pr_url.as_deref() != Some(&pr.url);
                if pr_changed {
                    topic_repo::update_topic_pr_info(conn, &topic.id, &pr.number, &pr.url)?;
                    updated += 1;
                }
            }
            None => {
                topic_repo::create_topic(
                    conn,
                    &repo.id,
                    &pr.head_branch,
                    BranchOrigin::Tracked,
                    Some(&pr.number),
                    Some(&pr.url),
                )?;
                created += 1;
            }
        }
    }

    Ok(SyncResult {
        created,
        updated,
        total_prs,
    })
}

// ---------------------------------------------------------------------------
// Refresh CI statuses
// ---------------------------------------------------------------------------

pub fn refresh_ci_statuses(conn: &Connection, repo: &Repo) -> Result<Vec<CiRefreshEntry>> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let topics = topic_repo::list_topics(conn, Some(&repo.id))?;

    let mut entries = Vec::new();

    for topic in &topics {
        if topic.status != TopicStatus::Active {
            continue;
        }

        let detail = match adapter.get_ci_status(&topic.branch) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let old_status = topic.ci_status;
        let new_status = detail.overall;

        // Find a representative URL from checks
        let ci_url = detail
            .checks
            .iter()
            .find_map(|c| c.url.as_deref())
            .or(detail.sha.as_deref());

        topic_repo::update_topic_ci_status(conn, &topic.id, new_status, ci_url)?;

        entries.push(CiRefreshEntry {
            topic_id: topic.id.clone(),
            branch: topic.branch.clone(),
            old_status,
            new_status,
        });
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Notify conflicts via PR comments
// ---------------------------------------------------------------------------

pub fn notify_conflicts(conn: &Connection, repo: &Repo, conflicts: &[Conflict]) -> Result<()> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());

    if !adapter.is_available() {
        return Ok(());
    }

    for conflict in conflicts {
        let topic = match topic_repo::get_topic(conn, &conflict.topic_id) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let pr_number = match topic.pr_id.as_deref() {
            Some(n) => n,
            None => continue,
        };

        let body = format!(
            "**Restack conflict detected**\n\n\
             Branch `{}` conflicted during rebuild{}.\n\
             The topic has been removed from the environment and marked as conflicting.\n\
             Please rebase or resolve conflicts and re-add the topic.",
            topic.branch,
            conflict
                .conflicted_with
                .as_deref()
                .map(|w| format!(" with `{w}`"))
                .unwrap_or_default(),
        );

        if let Err(e) = adapter.comment_on_pr(pr_number, &body) {
            eprintln!(
                "Warning: failed to comment on PR {} for conflict: {e}",
                pr_number
            );
        }
    }

    Ok(())
}

/// Helper to load a repo by ID string (used by CLI commands).
pub fn load_repo(conn: &Connection, repo_id_str: &str) -> Result<Repo> {
    let repo_id = repo_id_str
        .parse()
        .map_err(|_| crate::error::RestackError::RepoNotFound(crate::id::RepoId::new()))?;
    repo_repo::get_repo(conn, &repo_id)
}
