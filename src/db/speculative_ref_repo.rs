use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;
use crate::id::{EnvId, RebuildId, SpeculativeRefId, TopicId};
use crate::types::{CiStatus, SpeculativeRef};

pub fn create_speculative_ref(
    conn: &Connection,
    rebuild_id: &RebuildId,
    env_id: &EnvId,
    step: i32,
    topic_id: &TopicId,
    sha: &str,
    branch_name: &str,
) -> Result<SpeculativeRef> {
    let id = SpeculativeRefId::new();
    let created_at = Utc::now();
    let created_at_str = created_at.to_rfc3339();

    conn.execute(
        "INSERT INTO speculative_refs (id, rebuild_id, env_id, step, topic_id, sha, branch_name, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, rebuild_id, env_id, step, topic_id, sha, branch_name, created_at_str],
    )?;

    Ok(SpeculativeRef {
        id,
        rebuild_id: rebuild_id.clone(),
        env_id: env_id.clone(),
        step,
        topic_id: topic_id.clone(),
        sha: sha.to_string(),
        branch_name: branch_name.to_string(),
        ci_status: None,
        ci_url: None,
        created_at,
    })
}

pub fn get_refs_for_rebuild(
    conn: &Connection,
    rebuild_id: &RebuildId,
) -> Result<Vec<SpeculativeRef>> {
    let mut stmt = conn.prepare(
        "SELECT id, rebuild_id, env_id, step, topic_id, sha, branch_name, ci_status, ci_url, created_at FROM speculative_refs WHERE rebuild_id = ?1 ORDER BY step",
    )?;

    let rows = stmt.query_map([rebuild_id], map_row)?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn get_refs_for_env(conn: &Connection, env_id: &EnvId) -> Result<Vec<SpeculativeRef>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT s.id, s.rebuild_id, s.env_id, s.step, s.topic_id, s.sha, s.branch_name, s.ci_status, s.ci_url, s.created_at
        FROM speculative_refs s
        INNER JOIN rebuilds r ON s.rebuild_id = r.id
        WHERE s.env_id = ?1
          AND r.id = (
              SELECT id FROM rebuilds WHERE env_id = ?1 ORDER BY started_at DESC LIMIT 1
          )
        ORDER BY s.step
        "#,
    )?;

    let rows = stmt.query_map([env_id], map_row)?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn set_ci_status(
    conn: &Connection,
    specref_id: &SpeculativeRefId,
    ci_status: CiStatus,
    ci_url: Option<&str>,
) -> Result<()> {
    let status_str = match ci_status {
        CiStatus::Pending => "pending",
        CiStatus::Passed => "passed",
        CiStatus::Failed => "failed",
    };
    conn.execute(
        "UPDATE speculative_refs SET ci_status = ?1, ci_url = ?2 WHERE id = ?3",
        rusqlite::params![status_str, ci_url, specref_id],
    )?;
    Ok(())
}

pub fn delete_refs_for_rebuild(conn: &Connection, rebuild_id: &RebuildId) -> Result<()> {
    conn.execute(
        "DELETE FROM speculative_refs WHERE rebuild_id = ?1",
        [rebuild_id],
    )?;
    Ok(())
}

fn parse_ci_status(s: &str) -> Option<CiStatus> {
    match s {
        "pending" => Some(CiStatus::Pending),
        "passed" => Some(CiStatus::Passed),
        "failed" => Some(CiStatus::Failed),
        _ => None,
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SpeculativeRef> {
    let ci_status = row
        .get::<_, Option<String>>(7)?
        .and_then(|s| parse_ci_status(&s));
    let created_at = row.get::<_, String>(9).and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    9,
                    "created_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })
    })?;

    Ok(SpeculativeRef {
        id: row.get(0)?,
        rebuild_id: row.get(1)?,
        env_id: row.get(2)?,
        step: row.get(3)?,
        topic_id: row.get(4)?,
        sha: row.get(5)?,
        branch_name: row.get(6)?,
        ci_status,
        ci_url: row.get(8)?,
        created_at,
    })
}
