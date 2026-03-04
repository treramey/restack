use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;
use crate::id::{EnvId, RebuildId};
use crate::types::{Rebuild, RebuildStatus};

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
        "SELECT id, env_id, started_at, completed_at, status, topics_merged, topics_conflicted, result_sha FROM rebuilds WHERE env_id = ?1 ORDER BY started_at DESC LIMIT 1",
    )?;

    let mut rows = stmt.query([env_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(map_rebuild_row(row)?)),
        None => Ok(None),
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

    Ok(Rebuild {
        id: row.get(0)?,
        env_id: row.get(1)?,
        started_at,
        completed_at,
        status,
        topics_merged: row.get(5)?,
        topics_conflicted: row.get(6)?,
        result_sha: row.get(7)?,
    })
}
