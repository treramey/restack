use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{env_repo, repo_repo};
use crate::error::{RestackError, Result};
use crate::git;
use crate::types::{Provider, Repo};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedRepo {
    pub path: String,
    pub name: String,
    pub provider: Provider,
    pub already_tracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectResult {
    pub found: Vec<DetectedRepo>,
    pub added: Vec<Repo>,
    pub skipped: usize,
}

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
        return Err(RestackError::RepoAlreadyTracked(canonical_path));
    }

    let base_branch = git::detect_default_branch(path);
    let repo = repo_repo::create_repo(conn, &repo_name, &canonical_path, None, provider, &base_branch)?;

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

/// Walk subdirectories (1-2 levels deep) looking for .git directories,
/// detect provider, filter already-tracked repos, and add new ones.
pub fn detect_repos(conn: &Connection, workspace_root: &Path) -> Result<DetectResult> {
    let tracked = repo_repo::list_repos(conn)?;
    let tracked_paths: Vec<String> = tracked.iter().map(|r| r.path.clone()).collect();

    let mut found = Vec::new();

    // Walk 1-2 levels deep
    for entry in walk_git_dirs(workspace_root, 2) {
        let canonical = entry
            .canonicalize()
            .unwrap_or_else(|_| entry.clone())
            .to_string_lossy()
            .to_string();

        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();

        let already_tracked = tracked_paths.contains(&canonical);
        let provider = git::detect_provider(&entry).unwrap_or(Provider::Unknown);

        found.push(DetectedRepo {
            path: canonical,
            name,
            provider,
            already_tracked,
        });
    }

    // Add untracked repos
    let mut added = Vec::new();
    let mut skipped = 0;

    for detected in &found {
        if detected.already_tracked {
            skipped += 1;
            continue;
        }

        let base_branch = git::detect_default_branch(Path::new(&detected.path));
        let repo = repo_repo::create_repo(
            conn,
            &detected.name,
            &detected.path,
            None,
            detected.provider,
            &base_branch,
        )?;

        // Create default environments
        env_repo::create_env(conn, &repo.id, "staging", "staging", 0, false)?;
        env_repo::create_env(conn, &repo.id, "dev", "dev", 1, true)?;

        added.push(repo);
    }

    Ok(DetectResult {
        found,
        added,
        skipped,
    })
}

/// Recursively find directories containing `.git` up to `max_depth` levels.
fn walk_git_dirs(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut results = Vec::new();
    walk_git_dirs_inner(root, max_depth, 0, &mut results);
    results
}

fn walk_git_dirs_inner(dir: &Path, max_depth: usize, depth: usize, results: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip hidden dirs (except .git check below)
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        if path.join(".git").exists() {
            results.push(path);
        } else if depth < max_depth {
            walk_git_dirs_inner(&path, max_depth, depth + 1, results);
        }
    }
}
