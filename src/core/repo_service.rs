use std::path::Path;

use rusqlite::Connection;

use crate::db::{env_repo, repo_repo};
use crate::error::{RestackError, Result};
use crate::git;
use crate::types::{Provider, Repo};

pub fn add_repo(
    conn: &Connection,
    _workspace_root: &Path,
    repo_path: &str,
    name: Option<&str>,
) -> Result<Repo> {
    let path = Path::new(repo_path);

    // Validate path exists and is a git repo
    if !path.exists() {
        return Err(RestackError::NotInGitRepo);
    }

    // Detect provider from remote
    let provider = git::detect_provider(path)
        .unwrap_or(Provider::Unknown);

    // Use directory name as default name
    let repo_name = name.map(|n| n.to_string()).unwrap_or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    let canonical_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();

    // Check if already registered
    if repo_repo::get_repo_by_path(conn, &canonical_path)?.is_some() {
        return Err(RestackError::TopicAlreadyTracked {
            branch: format!("repo at {}", canonical_path),
        });
    }

    let repo = repo_repo::create_repo(conn, &repo_name, &canonical_path, None, provider, "main")?;

    // Create default environments
    env_repo::create_env(conn, &repo.id, "staging", "staging", 0, false)?;
    env_repo::create_env(conn, &repo.id, "dev", "dev", 1, true)?;

    Ok(repo)
}

pub fn remove_repo(conn: &Connection, id_or_name: &str) -> Result<()> {
    // Try as ID first, then by name
    if let Ok(id) = id_or_name.parse() {
        return repo_repo::delete_repo(conn, &id);
    }

    match repo_repo::get_repo_by_name(conn, id_or_name)? {
        Some(repo) => repo_repo::delete_repo(conn, &repo.id),
        None => Err(RestackError::RepoNotFound(
            id_or_name.parse().unwrap_or_default(),
        )),
    }
}

pub fn list_repos(conn: &Connection) -> Result<Vec<Repo>> {
    repo_repo::list_repos(conn)
}
