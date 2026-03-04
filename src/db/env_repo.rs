use rusqlite::Connection;

use crate::error::{RestackError, Result};
use crate::id::{EnvId, RepoId};
use crate::types::Environment;

pub fn create_env(
    conn: &Connection,
    repo_id: &RepoId,
    name: &str,
    branch: &str,
    ordinal: i32,
    auto_promote: bool,
) -> Result<Environment> {
    let id = EnvId::new();

    conn.execute(
        "INSERT INTO environments (id, repo_id, name, branch, ordinal, auto_promote) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, repo_id, name, branch, ordinal, auto_promote as i32],
    )?;

    Ok(Environment {
        id,
        repo_id: repo_id.clone(),
        name: name.to_string(),
        branch: branch.to_string(),
        ordinal,
        auto_promote,
    })
}

pub fn get_env(conn: &Connection, id: &EnvId) -> Result<Environment> {
    conn.query_row(
        "SELECT id, repo_id, name, branch, ordinal, auto_promote FROM environments WHERE id = ?1",
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
        "SELECT id, repo_id, name, branch, ordinal, auto_promote FROM environments WHERE repo_id = ?1 AND name = ?2",
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
                "SELECT id, repo_id, name, branch, ordinal, auto_promote FROM environments WHERE repo_id = ?1 ORDER BY ordinal",
            )?;
            let rows = stmt.query_map([rid], map_env_row)?;
            for row in rows {
                envs.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, repo_id, name, branch, ordinal, auto_promote FROM environments ORDER BY ordinal",
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

fn map_env_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Environment> {
    Ok(Environment {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        name: row.get(2)?,
        branch: row.get(3)?,
        ordinal: row.get(4)?,
        auto_promote: row.get::<_, i32>(5)? != 0,
    })
}
