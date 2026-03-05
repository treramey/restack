use std::path::Path;

use rusqlite::Connection;

use crate::config;
use crate::core::{discovery_service, provider_service};
use crate::db::repo_repo;
use crate::error::Result;

pub fn handle_refresh(
    conn: &Connection,
    repo_id: Option<&str>,
    workspace_root: &Path,
) -> Result<String> {
    let config_path = workspace_root.join(".restack/config.toml");
    let cfg = if config_path.exists() {
        config::load_config(&config_path)?
    } else {
        config::default_config()
    };

    let repos = if let Some(id) = repo_id {
        let rid = id
            .parse()
            .map_err(|_| crate::error::RestackError::RepoNotFound(crate::id::RepoId::new()))?;
        vec![repo_repo::get_repo(conn, &rid)?]
    } else {
        repo_repo::list_repos(conn)?
    };

    let mut results = Vec::new();

    for repo in &repos {
        let repo_path = Path::new(&repo.path);

        let discovery = discovery_service::discover_topics(conn, &repo.id, repo_path, &cfg)?;

        let ci_refresh = if cfg.provider.auto_ci_refresh {
            provider_service::refresh_ci_statuses(conn, repo)?
        } else {
            Vec::new()
        };

        results.push(serde_json::json!({
            "repo": repo.name,
            "discovery": discovery,
            "ci_refreshed": ci_refresh.len(),
        }));
    }

    Ok(serde_json::to_string_pretty(&results)?)
}
