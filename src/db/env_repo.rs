use chrono::Utc;
use rusqlite::Connection;

use crate::error::{RestackError, Result};
use crate::id::{EnvId, RepoId};
use crate::types::{CiStatus, Environment};

pub fn create_env(
    conn: &Connection,
    repo_id: &RepoId,
    name: &str,
    branch: &str,
    ordinal: i32,
) -> Result<Environment> {
    let id = EnvId::new();

    conn.execute(
        "INSERT INTO environments (id, repo_id, name, branch, ordinal) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, repo_id, name, branch, ordinal],
    )?;

    Ok(Environment {
        id,
        repo_id: repo_id.clone(),
        name: name.to_string(),
        branch: branch.to_string(),
        ordinal,
        ci_status: None,
        ci_url: None,
        last_ci_check: None,
    })
}

pub fn get_env(conn: &Connection, id: &EnvId) -> Result<Environment> {
    conn.query_row(
        "SELECT id, repo_id, name, branch, ordinal, ci_status, ci_url, last_ci_check FROM environments WHERE id = ?1",
        [id],
        map_env_row,
    )
    .map_err(|_| RestackError::EnvNotFound(id.clone()))
}

pub fn get_env_by_name(
    conn: &Connection,
    repo_id: &RepoId,
    name: &str,
) -> Result<Option<Environment>> {
    let mut stmt = conn.prepare(
        "SELECT id, repo_id, name, branch, ordinal, ci_status, ci_url, last_ci_check FROM environments WHERE repo_id = ?1 AND name = ?2",
    )?;

    let mut rows = stmt.query(rusqlite::params![repo_id, name])?;
    match rows.next()? {
        Some(row) => Ok(Some(map_env_row(row)?)),
        None => Ok(None),
    }
}

pub fn list_envs(conn: &Connection, repo_id: Option<&RepoId>) -> Result<Vec<Environment>> {
    let mut envs = Vec::new();

    match repo_id {
        Some(rid) => {
            let mut stmt = conn.prepare(
                "SELECT id, repo_id, name, branch, ordinal, ci_status, ci_url, last_ci_check FROM environments WHERE repo_id = ?1 ORDER BY ordinal",
            )?;
            let rows = stmt.query_map([rid], map_env_row)?;
            for row in rows {
                envs.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, repo_id, name, branch, ordinal, ci_status, ci_url, last_ci_check FROM environments ORDER BY ordinal",
            )?;
            let rows = stmt.query_map([], map_env_row)?;
            for row in rows {
                envs.push(row?);
            }
        }
    }

    Ok(envs)
}

pub fn delete_env(conn: &Connection, id: &EnvId) -> Result<()> {
    let affected = conn.execute("DELETE FROM environments WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(RestackError::EnvNotFound(id.clone()));
    }
    Ok(())
}

pub fn update_env(
    conn: &Connection,
    env_id: &EnvId,
    branch: &str,
    ordinal: i32,
) -> Result<()> {
    let affected = conn.execute(
        "UPDATE environments SET branch = ?1, ordinal = ?2 WHERE id = ?3",
        rusqlite::params![branch, ordinal, env_id],
    )?;
    if affected == 0 {
        return Err(RestackError::EnvNotFound(env_id.clone()));
    }
    Ok(())
}

pub fn count_topics_in_env(conn: &Connection, env_id: &EnvId) -> Result<i32> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM topic_environments WHERE env_id = ?1",
        [env_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn set_env_ci_status(
    conn: &Connection,
    env_id: &EnvId,
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
        "UPDATE environments SET ci_status = ?1, ci_url = ?2, last_ci_check = ?3 WHERE id = ?4",
        rusqlite::params![status_str, ci_url, now, env_id],
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

fn map_env_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Environment> {
    let ci_status = row
        .get::<_, Option<String>>(5)?
        .and_then(|s| parse_ci_status(&s));
    let ci_url = row.get(6)?;
    let last_ci_check = row
        .get::<_, Option<String>>(7)?
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(Environment {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        name: row.get(2)?,
        branch: row.get(3)?,
        ordinal: row.get(4)?,
        ci_status,
        ci_url,
        last_ci_check,
    })
}
