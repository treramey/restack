use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::core::discovery_service;
use crate::db::repo_repo;
use crate::error::{RestackError, Result};
use crate::git;
use crate::id::RepoId;
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
    workspace_root: &Path,
    repo_path: &str,
    name: Option<&str>,
    discover: bool,
) -> Result<serde_json::Value> {
    let path = Path::new(repo_path);

    // Validate path exists and is a git repo
    if !path.exists() {
        return Err(RestackError::NotInGitRepo);
    }

    // Detect provider and remote URL
    let provider = git::detect_provider(path).unwrap_or(Provider::Unknown);
    let remote_url = git::get_remote_url(path);

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
    let repo = repo_repo::create_repo(
        conn,
        &repo_name,
        &canonical_path,
        remote_url.as_deref(),
        provider,
        &base_branch,
    )?;

    let mut result = serde_json::json!({
        "repo": repo,
    });

    let restack_yml_path = path.join(".restack.yml");
    if restack_yml_path.exists() {
        match crate::config::repo_config::load_repo_config(&restack_yml_path) {
            Ok(config) => {
                if let Err(e) = crate::config::repo_config::validate_version(&config) {
                    result["env_config_error"] = e.to_string().into();
                } else if let Err(e) =
                    crate::config::repo_config::validate_no_duplicate_branches(&config)
                {
                    result["env_config_error"] = e.to_string().into();
                } else if let Err(e) =
                    crate::config::repo_config::validate_production_branch_collision(
                        &config,
                        &base_branch,
                    )
                {
                    result["env_config_error"] = e.to_string().into();
                } else {
                    let summary = crate::core::env_sync_service::reconcile_environments(
                        conn, &repo.id, &config,
                    )?;
                    result["env_reconcile"] = serde_json::to_value(&summary)?;
                }
            }
            Err(e) => {
                result["env_config_error"] = e.to_string().into();
            }
        }
    } else {
        // Generate default .restack.yml and reconcile
        std::fs::write(&restack_yml_path, crate::config::repo_config::DEFAULT_RESTACK_YML)?;
        let config = crate::config::repo_config::load_repo_config(&restack_yml_path)?;
        let summary =
            crate::core::env_sync_service::reconcile_environments(conn, &repo.id, &config)?;
        result["env_reconcile"] = serde_json::to_value(&summary)?;
    }

    // Handle discovery if requested
    if discover {
        let config_path = workspace_root.join(".restack/config.toml");
        let cfg = if config_path.exists() {
            config::load_config(&config_path)?
        } else {
            config::default_config()
        };

        match discovery_service::discover_topics(conn, &repo.id, path, &cfg) {
            Ok(discovery) => {
                result["discovery"] = serde_json::to_value(&discovery)?;
            }
            Err(e) => {
                result["discovery_error"] = serde_json::Value::String(e.to_string());
            }
        }
    } else {
        result["hint"] = "Use --discover to scan for topics after adding the repo".into();
    }

    Ok(result)
}

pub fn remove_repo(conn: &Connection, id_or_name: &str) -> Result<()> {
    let id_or_name = id_or_name.trim_end_matches('/');
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

    // Walk only immediate children of the workspace root.
    // Nested repos (worktrees, org dirs like LAAIR_Services/) are
    // excluded — use `repo add <path>` for those.
    for entry in walk_git_dirs(workspace_root, 0) {
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

        // Double-check DB to avoid UNIQUE constraint violation when
        // in-memory path comparison diverges from the stored path.
        if repo_repo::get_repo_by_path(conn, &detected.path)?.is_some() {
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

        // Generate default .restack.yml and reconcile
        let repo_path = Path::new(&detected.path);
        let restack_yml_path = repo_path.join(".restack.yml");
        if !restack_yml_path.exists() {
            std::fs::write(&restack_yml_path, crate::config::repo_config::DEFAULT_RESTACK_YML)?;
        }
        let config = crate::config::repo_config::load_repo_config(&restack_yml_path)?;
        crate::core::env_sync_service::reconcile_environments(conn, &repo.id, &config)?;

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

fn find_git_repo_root(cwd: &Path) -> Option<PathBuf> {
    let mut current = cwd;
    loop {
        if current.join(".git").exists() {
            return current.canonicalize().ok();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

pub fn find_repo_from_cwd(conn: &Connection, cwd: &Path) -> Result<Option<Repo>> {
    let git_root = match find_git_repo_root(cwd) {
        Some(path) => path,
        None => return Ok(None),
    };

    let path_str = git_root.to_string_lossy().to_string();
    repo_repo::get_repo_by_path(conn, &path_str)
}

/// Resolve repo from explicit arg or CWD auto-detection
///
/// Resolution order:
/// 1. If explicit_repo provided:
///    a. Try parse as RepoId (long ID)
///    b. Try lookup by name
/// 2. If no explicit_repo:
///    a. Walk up from cwd to find .git
///    b. Match path against tracked repos
///    c. Error if not in tracked repo
pub fn resolve_repo(conn: &Connection, explicit_repo: Option<&str>, cwd: &Path) -> Result<Repo> {
    // Case 1: Explicit repo argument provided
    if let Some(repo_arg) = explicit_repo {
        // 1a: Try as ID first
        if let Ok(id) = repo_arg.parse::<RepoId>() {
            return repo_repo::get_repo(conn, &id);
        }

        // 1b: Try as name
        match repo_repo::get_repo_by_name(conn, repo_arg)? {
            Some(repo) => return Ok(repo),
            None => return Err(RestackError::RepoNotFoundByName(repo_arg.to_string())),
        }
    }

    // Case 2: Auto-detect from CWD
    match find_repo_from_cwd(conn, cwd)? {
        Some(repo) => Ok(repo),
        None => Err(RestackError::NotInTrackedRepo),
    }
}

/// Find a repo that contains a specific branch.
///
/// Searches all tracked repos for a repo that has `branch` (locally or remotely).
/// Returns:
/// - Ok(repo) if the branch exists in exactly one repo
/// - Err(BranchNotFoundInAnyRepo) if the branch exists in no repos
/// - Err(BranchExistsInMultipleRepos) if the branch exists in multiple repos
pub fn resolve_repo_by_branch(conn: &Connection, branch: &str) -> Result<Repo> {
    let repos = repo_repo::list_repos(conn)?;

    if repos.is_empty() {
        return Err(RestackError::NoRepos);
    }

    // Use list_branch_presence (single git call per repo) instead of
    // branch_exists_anywhere (2 git calls per repo) to reduce subprocess spawns.
    let mut matching_repos: Vec<Repo> = Vec::new();

    for repo in repos {
        let repo_path = std::path::Path::new(&repo.path);
        match git::list_branch_presence(repo_path) {
            Ok(branches) => {
                if branches.iter().any(|b| b.branch == branch) {
                    matching_repos.push(repo);
                }
            }
            Err(_) => {}
        }
    }

    match matching_repos.len() {
        0 => Err(RestackError::BranchNotFoundInAnyRepo {
            branch: branch.to_string(),
        }),
        1 => Ok(matching_repos.into_iter().next().unwrap()),
        _ => {
            let repo_names: Vec<String> = matching_repos.iter().map(|r| r.name.clone()).collect();
            Err(RestackError::BranchExistsInMultipleRepos {
                branch: branch.to_string(),
                repos: repo_names.join(", "),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_add_repo_without_discover_includes_hint() {
        // Setup: create temp workspace with .restack
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        // Create a git repo to add
        let repo_dir = tempdir().expect("create repo dir");
        std::fs::create_dir(repo_dir.path().join(".git")).expect("create .git");

        // Setup DB
        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        // Call add_repo with discover=false
        let result = add_repo(
            &conn,
            workspace.path(),
            repo_dir.path().to_str().unwrap(),
            Some("test-repo"),
            false, // discover=false
        );

        // Assert success
        assert!(
            result.is_ok(),
            "add_repo should succeed: {:?}",
            result.err()
        );

        let json = result.unwrap();
        assert!(json.get("repo").is_some(), "should have repo field");
        assert!(
            json.get("hint").is_some(),
            "should have hint when discover=false"
        );
        assert!(
            json.get("discovery").is_none(),
            "should NOT have discovery when discover=false"
        );
    }

    #[test]
    fn test_add_repo_with_discover_includes_discovery() {
        // Setup: create temp workspace with .restack
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        // Create a real git repo with a branch
        let repo_dir = tempdir().expect("create repo dir");
        let repo_path = repo_dir.path();

        // Initialize git repo with a commit and feature branch
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["checkout", "-b", "feature/test"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Setup DB
        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        // Call add_repo with discover=true
        let result = add_repo(
            &conn,
            workspace.path(),
            repo_path.to_str().unwrap(),
            Some("test-repo"),
            true, // discover=true
        );

        // Assert success
        assert!(
            result.is_ok(),
            "add_repo should succeed: {:?}",
            result.err()
        );

        let json = result.unwrap();
        assert!(json.get("repo").is_some(), "should have repo field");
        assert!(
            json.get("discovery").is_some(),
            "should have discovery when discover=true"
        );
    }

    #[test]
    fn test_find_git_repo_root_finds_git() {
        let dir = tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir(&git_dir).unwrap();

        let found = find_git_repo_root(dir.path());
        assert!(found.is_some());
    }

    #[test]
    fn test_find_git_repo_root_walks_up() {
        let root = tempdir().unwrap();
        let git_dir = root.path().join(".git");
        std::fs::create_dir(&git_dir).unwrap();
        let nested = root.path().join("src/components");
        std::fs::create_dir_all(&nested).unwrap();

        let found = find_git_repo_root(&nested);
        assert_eq!(found, root.path().canonicalize().ok());
    }

    #[test]
    fn test_find_git_repo_root_not_found() {
        let dir = tempdir().unwrap();

        let found = find_git_repo_root(dir.path());
        assert!(found.is_none());
    }

    #[test]
    fn test_resolve_repo_by_id() {
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        let repo_dir = tempdir().expect("create repo dir");
        std::fs::create_dir(repo_dir.path().join(".git")).expect("create .git");

        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        let result = add_repo(
            &conn,
            workspace.path(),
            repo_dir.path().to_str().unwrap(),
            Some("test-repo"),
            false,
        );
        assert!(result.is_ok());
        let json = result.unwrap();
        let repo: Repo = serde_json::from_value(json["repo"].clone()).unwrap();

        let resolved = resolve_repo(&conn, Some(&repo.id.to_string()), workspace.path());
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap().id, repo.id);
    }

    #[test]
    fn test_resolve_repo_by_name() {
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        let repo_dir = tempdir().expect("create repo dir");
        std::fs::create_dir(repo_dir.path().join(".git")).expect("create .git");

        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        let result = add_repo(
            &conn,
            workspace.path(),
            repo_dir.path().to_str().unwrap(),
            Some("my-api"),
            false,
        );
        assert!(result.is_ok());

        let resolved = resolve_repo(&conn, Some("my-api"), workspace.path());
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap().name, "my-api");
    }

    #[test]
    fn test_resolve_repo_from_cwd() {
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        let repo_dir = tempdir().expect("create repo dir");
        std::fs::create_dir(repo_dir.path().join(".git")).expect("create .git");

        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        let result = add_repo(
            &conn,
            workspace.path(),
            repo_dir.path().to_str().unwrap(),
            Some("cwd-test-repo"),
            false,
        );
        assert!(result.is_ok());

        let resolved = resolve_repo(&conn, None, repo_dir.path());
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap().name, "cwd-test-repo");
    }

    #[test]
    fn test_resolve_repo_not_found_by_name() {
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        let resolved = resolve_repo(&conn, Some("nonexistent-repo"), workspace.path());
        assert!(resolved.is_err());
        assert!(matches!(
            resolved.unwrap_err(),
            RestackError::RepoNotFoundByName(_)
        ));
    }

    #[test]
    fn test_resolve_repo_not_in_tracked_repo() {
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        let untracked_dir = tempdir().expect("create untracked dir");
        std::fs::create_dir(untracked_dir.path().join(".git")).expect("create .git");

        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");

        let resolved = resolve_repo(&conn, None, untracked_dir.path());
        assert!(resolved.is_err());
        assert!(matches!(
            resolved.unwrap_err(),
            RestackError::NotInTrackedRepo
        ));
    }
}
