use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::id::TopicId;
use crate::provider::types::GeneratedFile;
use crate::provider::workflow;
use crate::types::{CiStatus, Provider, Repo};

use super::provider_service;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicCiStatus {
    pub topic_id: TopicId,
    pub branch: String,
    pub ci_status: Option<CiStatus>,
    pub ci_url: Option<String>,
    pub last_check: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowOutput {
    Stdout,
}

// ---------------------------------------------------------------------------
// Get CI status (delegates to provider_service)
// ---------------------------------------------------------------------------

pub fn get_ci_status(conn: &Connection, repo: &Repo) -> Result<Vec<TopicCiStatus>> {
    let entries = provider_service::refresh_ci_statuses(conn, repo)?;

    Ok(entries
        .into_iter()
        .map(|e| TopicCiStatus {
            topic_id: e.topic_id,
            branch: e.branch,
            ci_status: Some(e.new_status),
            ci_url: None,
            last_check: None,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Generate workflow files
// ---------------------------------------------------------------------------

pub fn generate_workflow(
    provider: Provider,
    output_dir: Option<&Path>,
    stdout: bool,
) -> Result<Vec<GeneratedFile>> {
    let files = workflow::generate_workflow_files(provider);

    if stdout {
        for file in &files {
            println!("# {}", file.path);
            println!("{}", file.content);
        }
    } else if let Some(dir) = output_dir {
        write_workflow_files(dir, &files)?;
    } else {
        let cwd = std::env::current_dir()?;
        write_workflow_files(&cwd, &files)?;
    }

    Ok(files)
}

fn write_workflow_files(base: &Path, files: &[GeneratedFile]) -> Result<()> {
    for file in files {
        let path = base.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.content)?;
    }
    Ok(())
}
