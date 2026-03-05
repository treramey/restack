use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;
use crate::id::{ConflictId, RebuildId, TopicId};
use crate::types::Conflict;

pub fn create_conflict(
    conn: &Connection,
    rebuild_id: &RebuildId,
    topic_id: &TopicId,
    conflicted_with: Option<&str>,
) -> Result<Conflict> {
    let id = ConflictId::new();
    let now = Utc::now();

    conn.execute(
        "INSERT INTO conflicts (id, rebuild_id, topic_id, conflicted_with, resolved, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, rebuild_id, topic_id, conflicted_with, 0, now.to_rfc3339()],
    )?;

    Ok(Conflict {
        id,
        rebuild_id: rebuild_id.clone(),
        topic_id: topic_id.clone(),
        conflicted_with: conflicted_with.map(|s| s.to_string()),
        resolved: false,
        created_at: now,
    })
}

pub fn list_conflicts(conn: &Connection, rebuild_id: &RebuildId) -> Result<Vec<Conflict>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, rebuild_id, topic_id, conflicted_with, resolved, created_at
           FROM conflicts
           WHERE rebuild_id = ?1
           ORDER BY created_at"#,
    )?;

    let rows = stmt.query_map([rebuild_id], |row| {
        Ok(ConflictRow {
            id: row.get(0)?,
            rebuild_id: row.get(1)?,
            topic_id: row.get(2)?,
            conflicted_with: row.get(3)?,
            resolved: row.get::<_, i32>(4)?,
            created_at: row.get::<_, String>(5)?,
        })
    })?;

    let mut conflicts = Vec::new();
    for row in rows {
        let r = row?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&r.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        conflicts.push(Conflict {
            id: r.id,
            rebuild_id: r.rebuild_id,
            topic_id: r.topic_id,
            conflicted_with: r.conflicted_with,
            resolved: r.resolved != 0,
            created_at,
        });
    }
    Ok(conflicts)
}

pub fn list_all_conflicts(conn: &Connection) -> Result<Vec<Conflict>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, rebuild_id, topic_id, conflicted_with, resolved, created_at
           FROM conflicts
           ORDER BY created_at DESC"#,
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ConflictRow {
            id: row.get(0)?,
            rebuild_id: row.get(1)?,
            topic_id: row.get(2)?,
            conflicted_with: row.get(3)?,
            resolved: row.get::<_, i32>(4)?,
            created_at: row.get::<_, String>(5)?,
        })
    })?;

    let mut conflicts = Vec::new();
    for row in rows {
        let r = row?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&r.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        conflicts.push(Conflict {
            id: r.id,
            rebuild_id: r.rebuild_id,
            topic_id: r.topic_id,
            conflicted_with: r.conflicted_with,
            resolved: r.resolved != 0,
            created_at,
        });
    }
    Ok(conflicts)
}

struct ConflictRow {
    id: ConflictId,
    rebuild_id: RebuildId,
    topic_id: TopicId,
    conflicted_with: Option<String>,
    resolved: i32,
    created_at: String,
}
