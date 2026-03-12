use std::path::Path;

use crate::config;
use crate::core::repo_service;
use crate::db;
use crate::error::Result;

pub fn handle_init(path: &Path) -> Result<String> {
    let restack_dir = path.join(".restack");
    std::fs::create_dir_all(&restack_dir)?;

    let config_path = restack_dir.join("config.toml");
    let _cfg = if config_path.exists() {
        config::load_config(&config_path)?
    } else {
        let cfg = config::default_config();
        config::save_config(&config_path, &cfg)?;
        cfg
    };

    let db_path = restack_dir.join("workspace.db");
    let conn = db::open_db(&db_path)?;

    let mut result = serde_json::json!({
        "initialized": true,
        "path": restack_dir.display().to_string()
    });

    if is_git_repo(path) {
        match repo_service::add_repo(&conn, path, &path.display().to_string(), None, true) {
            Ok(repo_result) => {
                if let Some(repo) = repo_result.get("repo") {
                    result["repo"] = repo.clone();
                }
                if let Some(discovery) = repo_result.get("discovery") {
                    result["discovery"] = discovery.clone();
                }
                if let Some(err) = repo_result.get("discovery_error") {
                    result["discovery_error"] = err.clone();
                }
                if let Some(hint) = repo_result.get("hint") {
                    result["hint"] = hint.clone();
                }
            }
            Err(e) => {
                result["repo_error"] = e.to_string().into();
            }
        }
    }

    Ok(result.to_string())
}

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}
