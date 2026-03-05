use chrono::Utc;
use rusqlite::Connection;

use crate::error::{RestackError, Result};
use crate::id::{RepoId, TopicId};
use crate::types::{CiStatus, Topic, TopicStatus};

pub fn create_topic(
    conn: &Connection,
    repo_id: &RepoId,
    branch: &str,
    pr_id: Option<&str>,
    pr_url: Option<&str>,
) -> Result<Topic> {
    let id = TopicId::new();
    let now = Utc::now();

    conn.execute(
        "INSERT INTO topics (id, repo_id, branch, pr_id, pr_url, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, repo_id, branch, pr_id, pr_url, "active", now.to_rfc3339()],
    )?;

    Ok(Topic {
        id,
        repo_id: repo_id.clone(),
        branch: branch.to_string(),
        pr_id: pr_id.map(|s| s.to_string()),
        pr_url: pr_url.map(|s| s.to_string()),
        status: TopicStatus::Active,
        ci_status: None,
        ci_url: None,
        last_ci_check: None,
        created_at: now,
    })
}

pub fn get_topic(conn: &Connection, id: &TopicId) -> Result<Topic> {
    conn.query_row(
        "SELECT id, repo_id, branch, pr_id, pr_url, status, ci_status, created_at, ci_url, last_ci_check FROM topics WHERE id = ?1",
        [id],
        |row| {
            Ok(TopicRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                branch: row.get(2)?,
                pr_id: row.get(3)?,
                pr_url: row.get(4)?,
                status: row.get::<_, String>(5)?,
                ci_status: row.get::<_, Option<String>>(6)?,
                created_at: row.get::<_, String>(7)?,
                ci_url: row.get::<_, Option<String>>(8)?,
                last_ci_check: row.get::<_, Option<String>>(9)?,
            })
        },
    )
    .map_err(|_| RestackError::TopicNotFound(id.clone()))
    .and_then(|r| r.into_topic())
}

pub fn get_topic_by_branch(
    conn: &Connection,
    repo_id: &RepoId,
    branch: &str,
) -> Result<Option<Topic>> {
    let mut stmt = conn.prepare(
        "SELECT id, repo_id, branch, pr_id, pr_url, status, ci_status, created_at, ci_url, last_ci_check FROM topics WHERE repo_id = ?1 AND branch = ?2",
    )?;

    let mut rows = stmt.query(rusqlite::params![repo_id, branch])?;
    match rows.next()? {
        Some(row) => {
            let r = TopicRow {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                branch: row.get(2)?,
                pr_id: row.get(3)?,
                pr_url: row.get(4)?,
                status: row.get::<_, String>(5)?,
                ci_status: row.get::<_, Option<String>>(6)?,
                created_at: row.get::<_, String>(7)?,
                ci_url: row.get::<_, Option<String>>(8)?,
                last_ci_check: row.get::<_, Option<String>>(9)?,
            };
            Ok(Some(r.into_topic()?))
        }
        None => Ok(None),
    }
}

pub fn list_topics(conn: &Connection, repo_id: Option<&RepoId>) -> Result<Vec<Topic>> {
    let mut topics = Vec::new();

    match repo_id {
        Some(rid) => {
            let mut stmt = conn.prepare(
                "SELECT id, repo_id, branch, pr_id, pr_url, status, ci_status, created_at, ci_url, last_ci_check FROM topics WHERE repo_id = ?1 ORDER BY branch",
            )?;
            let rows = stmt.query_map([rid], map_topic_row)?;
            for row in rows {
                topics.push(row?.into_topic()?);
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, repo_id, branch, pr_id, pr_url, status, ci_status, created_at, ci_url, last_ci_check FROM topics ORDER BY branch",
            )?;
            let rows = stmt.query_map([], map_topic_row)?;
            for row in rows {
                topics.push(row?.into_topic()?);
            }
        }
    }

    Ok(topics)
}

pub fn delete_topic(conn: &Connection, id: &TopicId) -> Result<()> {
    let affected = conn.execute("DELETE FROM topics WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(RestackError::TopicNotFound(id.clone()));
    }
    Ok(())
}

pub fn update_topic_status(conn: &Connection, id: &TopicId, status: TopicStatus) -> Result<()> {
    let status_str = match status {
        TopicStatus::Active => "active",
        TopicStatus::Conflict => "conflict",
        TopicStatus::Graduated => "graduated",
        TopicStatus::Closed => "closed",
    };
    conn.execute(
        "UPDATE topics SET status = ?1 WHERE id = ?2",
        rusqlite::params![status_str, id],
    )?;
    Ok(())
}

pub fn update_topic_status_return(
    conn: &Connection,
    id: &TopicId,
    status: TopicStatus,
) -> Result<Topic> {
    update_topic_status(conn, id, status)?;
    get_topic(conn, id)
}

fn map_topic_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TopicRow> {
    Ok(TopicRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        branch: row.get(2)?,
        pr_id: row.get(3)?,
        pr_url: row.get(4)?,
        status: row.get::<_, String>(5)?,
        ci_status: row.get::<_, Option<String>>(6)?,
        created_at: row.get::<_, String>(7)?,
        ci_url: row.get::<_, Option<String>>(8)?,
        last_ci_check: row.get::<_, Option<String>>(9)?,
    })
}

struct TopicRow {
    id: TopicId,
    repo_id: RepoId,
    branch: String,
    pr_id: Option<String>,
    pr_url: Option<String>,
    status: String,
    ci_status: Option<String>,
    created_at: String,
    ci_url: Option<String>,
    last_ci_check: Option<String>,
}

impl TopicRow {
    fn into_topic(self) -> Result<Topic> {
        let status = match self.status.as_str() {
            "active" => TopicStatus::Active,
            "conflict" => TopicStatus::Conflict,
            "graduated" => TopicStatus::Graduated,
            "closed" => TopicStatus::Closed,
            _ => TopicStatus::Active,
        };
        let ci_status = self.ci_status.as_deref().map(|s| match s {
            "passed" => CiStatus::Passed,
            "failed" => CiStatus::Failed,
            _ => CiStatus::Pending,
        });
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let last_ci_check = self
            .last_ci_check
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(Topic {
            id: self.id,
            repo_id: self.repo_id,
            branch: self.branch,
            pr_id: self.pr_id,
            pr_url: self.pr_url,
            status,
            ci_status,
            ci_url: self.ci_url,
            last_ci_check,
            created_at,
        })
    }
}

pub fn update_topic_ci_status(
    conn: &Connection,
    id: &TopicId,
    ci_status: CiStatus,
    ci_url: Option<&str>,
) -> Result<()> {
    let status_str = match ci_status {
        CiStatus::Pending => "pending",
        CiStatus::Passed => "passed",
        CiStatus::Failed => "failed",
    };
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE topics SET ci_status = ?1, ci_url = ?2, last_ci_check = ?3 WHERE id = ?4",
        rusqlite::params![status_str, ci_url, now, id],
    )?;
    Ok(())
}

pub fn update_topic_pr_info(
    conn: &Connection,
    id: &TopicId,
    pr_id: &str,
    pr_url: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE topics SET pr_id = ?1, pr_url = ?2 WHERE id = ?3",
        rusqlite::params![pr_id, pr_url, id],
    )?;
    Ok(())
}
