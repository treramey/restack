use std::path::Path;

use crate::config;
use crate::db;
use crate::error::Result;

pub fn handle_init(path: &Path) -> Result<String> {
    let restack_dir = path.join(".restack");
    std::fs::create_dir_all(&restack_dir)?;

    let config_path = restack_dir.join("config.toml");
    if !config_path.exists() {
        let cfg = config::default_config();
        config::save_config(&config_path, &cfg)?;
    }

    let db_path = restack_dir.join("workspace.db");
    db::open_db(&db_path)?;

    Ok(serde_json::json!({
        "initialized": true,
        "path": restack_dir.display().to_string()
    })
    .to_string())
}
