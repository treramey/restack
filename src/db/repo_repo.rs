use chrono::Utc;
use rusqlite::Connection;

use crate::error::{RestackError, Result};
use crate::id::RepoId;
use crate::types::{Provider, Repo};

pub fn create_repo(
    conn: &Connection,
    name: &str,
    path: &str,
    remote_url: Option<&str>,
    provider: Provider,
    base_branch: &str,
) -> Result<Repo> {
    let id = RepoId::new();
    let now = Utc::now();
    let provider_str = serde_json::to_value(provider)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    conn.execute(
        "INSERT INTO repos (id, name, path, remote_url, provider, base_branch, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, name, path, remote_url, &provider_str, base_branch, now.to_rfc3339()],
    )?;

    Ok(Repo {
        id,
        name: name.to_string(),
        path: path.to_string(),
        remote_url: remote_url.map(|s| s.to_string()),
        provider,
        base_branch: base_branch.to_string(),
        created_at: now,
        refs_fingerprint: None,
        last_refreshed_at: None,
    })
}

pub fn get_repo(conn: &Connection, id: &RepoId) -> Result<Repo> {
    conn.query_row(
        "SELECT id, name, path, remote_url, provider, base_branch, created_at, refs_fingerprint, last_refreshed_at FROM repos WHERE id = ?1",
        [id],
        |row| {
            Ok(RepoRow {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                remote_url: row.get(3)?,
                provider: row.get::<_, String>(4)?,
                base_branch: row.get(5)?,
                created_at: row.get::<_, String>(6)?,
                refs_fingerprint: row.get(7)?,
                last_refreshed_at: row.get(8)?,
            })
        },
    )
    .map_err(|_| RestackError::RepoNotFound(id.clone()))
    .and_then(|r| r.into_repo())
}

pub fn get_repo_by_name(conn: &Connection, name: &str) -> Result<Option<Repo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, remote_url, provider, base_branch, created_at, refs_fingerprint, last_refreshed_at FROM repos WHERE name = ?1",
    )?;

    let mut rows = stmt.query([name])?;
    match rows.next()? {
        Some(row) => {
            let r = RepoRow {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                remote_url: row.get(3)?,
                provider: row.get::<_, String>(4)?,
                base_branch: row.get(5)?,
                created_at: row.get::<_, String>(6)?,
                refs_fingerprint: row.get(7)?,
                last_refreshed_at: row.get(8)?,
            };
            Ok(Some(r.into_repo()?))
        }
        None => Ok(None),
    }
}

pub fn get_repo_by_path(conn: &Connection, path: &str) -> Result<Option<Repo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, remote_url, provider, base_branch, created_at, refs_fingerprint, last_refreshed_at FROM repos WHERE path = ?1",
    )?;

    let mut rows = stmt.query([path])?;
    match rows.next()? {
        Some(row) => {
            let r = RepoRow {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                remote_url: row.get(3)?,
                provider: row.get::<_, String>(4)?,
                base_branch: row.get(5)?,
                created_at: row.get::<_, String>(6)?,
                refs_fingerprint: row.get(7)?,
                last_refreshed_at: row.get(8)?,
            };
            Ok(Some(r.into_repo()?))
        }
        None => Ok(None),
    }
}

pub fn list_repos(conn: &Connection) -> Result<Vec<Repo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, remote_url, provider, base_branch, created_at, refs_fingerprint, last_refreshed_at FROM repos ORDER BY name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(RepoRow {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            remote_url: row.get(3)?,
            provider: row.get::<_, String>(4)?,
            base_branch: row.get(5)?,
            created_at: row.get::<_, String>(6)?,
            refs_fingerprint: row.get(7)?,
            last_refreshed_at: row.get(8)?,
        })
    })?;

    let mut repos = Vec::new();
    for row in rows {
        repos.push(row?.into_repo()?);
    }
    Ok(repos)
}

pub fn delete_repo(conn: &Connection, id: &RepoId) -> Result<()> {
    let affected = conn.execute("DELETE FROM repos WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(RestackError::RepoNotFound(id.clone()));
    }
    Ok(())
}

// Internal helper for row mapping
struct RepoRow {
    id: RepoId,
    name: String,
    path: String,
    remote_url: Option<String>,
    provider: String,
    base_branch: String,
    created_at: String,
    refs_fingerprint: Option<String>,
    last_refreshed_at: Option<String>,
}

impl RepoRow {
    fn into_repo(self) -> Result<Repo> {
        let provider = match self.provider.as_str() {
            "gitHub" | "github" | "GitHub" => Provider::GitHub,
            "azureDevOps" | "azure" | "AzureDevOps" => Provider::AzureDevOps,
            "bitbucket" | "Bitbucket" => Provider::Bitbucket,
            _ => Provider::Unknown,
        };
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Repo {
            id: self.id,
            name: self.name,
            path: self.path,
            remote_url: self.remote_url,
            provider,
            base_branch: self.base_branch,
            created_at,
            refs_fingerprint: self.refs_fingerprint,
            last_refreshed_at: self
                .last_refreshed_at
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }
}

pub fn update_repo_fingerprint(
    conn: &Connection,
    id: &RepoId,
    fingerprint: &str,
    refreshed_at: &str,
) -> Result<()> {
    let affected = conn.execute(
        "UPDATE repos SET refs_fingerprint = ?1, last_refreshed_at = ?2 WHERE id = ?3",
        rusqlite::params![fingerprint, refreshed_at, id],
    )?;
    if affected == 0 {
        return Err(RestackError::RepoNotFound(id.clone()));
    }
    Ok(())
}
