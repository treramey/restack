use std::path::{Path, PathBuf};

use crate::config::{self, WorkspaceConfig};
use crate::db;
use crate::error::{RestackError, Result};

const RESTACK_DIR: &str = ".restack";
const CONFIG_FILE: &str = "config.toml";
const DB_FILE: &str = "workspace.db";

pub fn init_workspace(path: &Path, config: &WorkspaceConfig) -> Result<()> {
    let restack_dir = path.join(RESTACK_DIR);
    std::fs::create_dir_all(&restack_dir)?;

    let config_path = restack_dir.join(CONFIG_FILE);
    config::save_config(&config_path, config)?;

    let db_path = restack_dir.join(DB_FILE);
    db::open_db(&db_path)?;

    Ok(())
}

pub fn find_workspace_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(RESTACK_DIR).is_dir() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(RestackError::NotAWorkspace);
        }
    }
}

pub fn resolve_db_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(RESTACK_DIR).join(DB_FILE)
}
