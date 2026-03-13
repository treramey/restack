use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;
use crate::id::{EnvId, RebuildId};
use crate::types::{CiStatus, Rebuild, RebuildStatus};

pub fn create_rebuild(conn: &Connection, env_id: &EnvId) -> Result<Rebuild> {
    let id = RebuildId::new();
    let now = Utc::now();

    conn.execute(
        "INSERT INTO rebuilds (id, env_id, started_at, status) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, env_id, now.to_rfc3339(), "running"],
    )?;

    Ok(Rebuild {
        id,
        env_id: env_id.clone(),
        started_at: now,
        completed_at: None,
        status: RebuildStatus::Running,
        topics_merged: 0,
        topics_conflicted: 0,
        result_sha: None,
        ci_status: None,
        ci_url: None,
        ci_checked_at: None,
        ci_retry_count: 0,
        ci_override: None,
    })
}

pub fn complete_rebuild(
    conn: &Connection,
    id: &RebuildId,
    status: RebuildStatus,
    topics_merged: i32,
    topics_conflicted: i32,
    result_sha: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    let status_str = match status {
        RebuildStatus::Running => "running",
        RebuildStatus::Success => "success",
        RebuildStatus::Partial => "partial",
        RebuildStatus::Failed => "failed",
    };

    conn.execute(
        "UPDATE rebuilds SET completed_at = ?1, status = ?2, topics_merged = ?3, topics_conflicted = ?4, result_sha = ?5 WHERE id = ?6",
        rusqlite::params![now.to_rfc3339(), status_str, topics_merged, topics_conflicted, result_sha, id],
    )?;

    Ok(())
}

pub fn get_last_rebuild(conn: &Connection, env_id: &EnvId) -> Result<Option<Rebuild>> {
    let mut stmt = conn.prepare(
        "SELECT id, env_id, started_at, completed_at, status, topics_merged, topics_conflicted, result_sha, ci_status, ci_url, ci_checked_at, ci_retry_count, ci_override FROM rebuilds WHERE env_id = ?1 ORDER BY started_at DESC LIMIT 1",
    )?;

    let mut rows = stmt.query([env_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(map_rebuild_row(row)?)),
        None => Ok(None),
    }
}

pub fn get_last_successful_ci_rebuild(
    conn: &Connection,
    env_id: &EnvId,
) -> Result<Option<Rebuild>> {
    let mut stmt = conn.prepare(
        "SELECT id, env_id, started_at, completed_at, status, topics_merged, topics_conflicted, result_sha, ci_status, ci_url, ci_checked_at, ci_retry_count, ci_override FROM rebuilds WHERE env_id = ?1 AND status = 'success' AND ci_status = 'passed' ORDER BY started_at DESC LIMIT 1",
    )?;

    let mut rows = stmt.query([env_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(map_rebuild_row(row)?)),
        None => Ok(None),
    }
}

pub fn list_rebuilds(conn: &Connection) -> Result<Vec<Rebuild>> {
    let mut stmt = conn.prepare(
        "SELECT id, env_id, started_at, completed_at, status, topics_merged, topics_conflicted, result_sha, ci_status, ci_url, ci_checked_at, ci_retry_count, ci_override FROM rebuilds ORDER BY started_at DESC",
    )?;

    let rows = stmt.query_map([], |row| map_rebuild_row(row))?;

    let mut rebuilds = Vec::new();
    for row in rows {
        rebuilds.push(row?);
    }
    Ok(rebuilds)
}

pub fn set_rebuild_ci_status(
    conn: &Connection,
    rebuild_id: &RebuildId,
    ci_status: Option<CiStatus>,
    ci_url: Option<&str>,
) -> Result<()> {
    let status_str = ci_status.map(|s| match s {
        CiStatus::Pending => "pending",
        CiStatus::Passed => "passed",
        CiStatus::Failed => "failed",
    });
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE rebuilds SET ci_status = ?1, ci_url = ?2, ci_checked_at = ?3 WHERE id = ?4",
        rusqlite::params![status_str, ci_url, now, rebuild_id],
    )?;
    Ok(())
}

pub fn increment_rebuild_ci_retry(conn: &Connection, rebuild_id: &RebuildId) -> Result<i32> {
    conn.execute(
        "UPDATE rebuilds SET ci_retry_count = ci_retry_count + 1 WHERE id = ?1",
        rusqlite::params![rebuild_id],
    )?;
    let count: i32 = conn.query_row(
        "SELECT ci_retry_count FROM rebuilds WHERE id = ?1",
        rusqlite::params![rebuild_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

fn parse_ci_status(s: &str) -> Option<CiStatus> {
    match s {
        "pending" => Some(CiStatus::Pending),
        "passed" => Some(CiStatus::Passed),
        "failed" => Some(CiStatus::Failed),
        _ => None,
    }
}

fn map_rebuild_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Rebuild> {
    let status_str: String = row.get(4)?;
    let status = match status_str.as_str() {
        "success" => RebuildStatus::Success,
        "partial" => RebuildStatus::Partial,
        "failed" => RebuildStatus::Failed,
        _ => RebuildStatus::Running,
    };

    let started_str: String = row.get(2)?;
    let started_at = chrono::DateTime::parse_from_rfc3339(&started_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let completed_at = row
        .get::<_, Option<String>>(3)?
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let ci_status = row
        .get::<_, Option<String>>(8)?
        .and_then(|s| parse_ci_status(&s));
    let ci_url = row.get(9)?;
    let ci_checked_at = row
        .get::<_, Option<String>>(10)?
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let ci_retry_count = row.get::<_, Option<i32>>(11)?.unwrap_or(0);
    let ci_override = row
        .get::<_, Option<String>>(12)?
        .and_then(|s| parse_ci_status(&s));

    Ok(Rebuild {
        id: row.get(0)?,
        env_id: row.get(1)?,
        started_at,
        completed_at,
        status,
        topics_merged: row.get(5)?,
        topics_conflicted: row.get(6)?,
        result_sha: row.get(7)?,
        ci_status,
        ci_url,
        ci_checked_at,
        ci_retry_count,
        ci_override,
    })
}
