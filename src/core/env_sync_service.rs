use std::collections::HashMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::repo_config::RepoConfig;
use crate::db::{env_repo, topic_env_repo, topic_repo};
use crate::error::Result;
use crate::id::RepoId;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileSummary {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub blocked_removals: Vec<BlockedRemoval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedRemoval {
    pub env_name: String,
    pub reason: String,
}

impl ReconcileSummary {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.blocked_removals.is_empty()
    }
}

/// Format a reconcile summary for display to user.
pub fn format_reconcile_summary(summary: &ReconcileSummary) -> String {
    let mut lines = vec!["ℹ Reconciled environments from .restack.yml:".to_string()];

    for added in &summary.added {
        lines.push(format!("  + Added: {}", added));
    }
    for updated in &summary.updated {
        lines.push(format!("  ~ Updated: {}", updated));
    }
    for removed in &summary.removed {
        lines.push(format!("  - Removed: {}", removed));
    }
    for blocked in &summary.blocked_removals {
        lines.push(format!(
            "  ⚠ Cannot remove {} ({})",
            blocked.env_name, blocked.reason
        ));
    }

    lines.join("\n")
}

/// Reconcile environments for a repo if .restack.yml exists.
/// Returns summary of changes (None if no changes or no YAML).
pub fn maybe_reconcile_repo_envs(
    conn: &Connection,
    repo_id: &RepoId,
    repo_path: &std::path::Path,
) -> Result<Option<ReconcileSummary>> {
    let restack_yml_path = repo_path.join(".restack.yml");
    if !restack_yml_path.exists() {
        return Ok(None);
    }

    let config = crate::config::repo_config::load_repo_config(&restack_yml_path)?;
    crate::config::repo_config::validate_version(&config)?;

    let summary = reconcile_environments(conn, repo_id, &config)?;

    if summary.is_empty() {
        Ok(None)
    } else {
        Ok(Some(summary))
    }
}

pub fn reconcile_environments(
    conn: &Connection,
    repo_id: &RepoId,
    config: &RepoConfig,
) -> Result<ReconcileSummary> {
    let mut summary = ReconcileSummary::default();

    let existing_envs = env_repo::list_envs(conn, Some(repo_id))?;
    let existing_map: HashMap<String, _> = existing_envs
        .iter()
        .map(|e| (e.name.clone(), e.clone()))
        .collect();

    for (ordinal, entry) in config.environments.iter().enumerate() {
        let env_name = entry.name();
        let branch = entry.branch();
        let ordinal = ordinal as i32;

        match existing_map.get(env_name) {
            None => {
                env_repo::create_env(conn, repo_id, env_name, &branch, ordinal)?;
                summary.added.push(env_name.to_string());
            }
            Some(existing) => {
                let needs_update =
                    existing.branch != branch || existing.ordinal != ordinal;

                if needs_update {
                    env_repo::update_env(conn, &existing.id, &branch, ordinal)?;
                    summary.updated.push(env_name.to_string());
                }
            }
        }
    }

    // Remove any topics whose branch matches an environment branch
    for entry in &config.environments {
        let branch = entry.branch();
        if let Some(topic) = topic_repo::get_topic_by_branch(conn, repo_id, &branch)? {
            topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;
            topic_repo::delete_topic(conn, &topic.id)?;
        }
    }

    let config_env_names: std::collections::HashSet<_> =
        config.environments.iter().map(|e| e.name()).collect();

    for existing in &existing_envs {
        if !config_env_names.contains(existing.name.as_str()) {
            let topic_count = env_repo::count_topics_in_env(conn, &existing.id)?;
            if topic_count == 0 {
                env_repo::delete_env(conn, &existing.id)?;
                summary.removed.push(existing.name.clone());
            } else {
                summary.blocked_removals.push(BlockedRemoval {
                    env_name: existing.name.clone(),
                    reason: format!("{} topics assigned", topic_count),
                });
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_db() -> (tempfile::TempDir, Connection) {
        let workspace = tempdir().expect("create temp dir");
        let restack_dir = workspace.path().join(".restack");
        std::fs::create_dir_all(&restack_dir).expect("create .restack");

        let db_path = restack_dir.join("workspace.db");
        let conn = crate::db::schema::open_db(&db_path).expect("open db");
        (workspace, conn)
    }

    fn create_test_repo(conn: &Connection) -> RepoId {
        let repo = crate::db::repo_repo::create_repo(
            conn,
            "test-repo",
            "/test/path",
            None,
            crate::types::Provider::Unknown,
            "main",
        )
        .expect("create repo");
        repo.id
    }

    #[test]
    fn test_reconcile_adds_new_environments() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        let yaml = r#"version: "1"
environments:
  - dev
  - staging"#;
        let config = crate::config::repo_config::parse_repo_config(yaml).unwrap();

        let summary = reconcile_environments(&conn, &repo_id, &config).unwrap();

        assert!(summary.added.contains(&"dev".to_string()));
        assert!(summary.added.contains(&"staging".to_string()));
        assert_eq!(summary.added.len(), 2);
        assert!(summary.updated.is_empty());
        assert!(summary.removed.is_empty());
        assert!(summary.blocked_removals.is_empty());

        let envs = env_repo::list_envs(&conn, Some(&repo_id)).unwrap();
        assert_eq!(envs.len(), 2);
    }

    #[test]
    fn test_reconcile_updates_changed_environments() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        env_repo::create_env(&conn, &repo_id, "dev", "dev", 0).unwrap();

        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: develop"#;
        let config = crate::config::repo_config::parse_repo_config(yaml).unwrap();

        let summary = reconcile_environments(&conn, &repo_id, &config).unwrap();

        assert!(summary.added.is_empty());
        assert_eq!(summary.updated, vec!["dev"]);
        assert!(summary.removed.is_empty());

        let envs = env_repo::list_envs(&conn, Some(&repo_id)).unwrap();
        let dev = &envs[0];
        assert_eq!(dev.branch, "develop");
        assert_eq!(dev.ordinal, 0);
    }

    #[test]
    fn test_reconcile_removes_environments_with_no_topics() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        env_repo::create_env(&conn, &repo_id, "old-env", "old-env", 0).unwrap();

        let yaml = r#"version: "1"
environments: []"#;
        let config = crate::config::repo_config::parse_repo_config(yaml).unwrap();

        let summary = reconcile_environments(&conn, &repo_id, &config).unwrap();

        assert!(summary.added.is_empty());
        assert!(summary.updated.is_empty());
        assert_eq!(summary.removed, vec!["old-env"]);
        assert!(summary.blocked_removals.is_empty());

        let envs = env_repo::list_envs(&conn, Some(&repo_id)).unwrap();
        assert!(envs.is_empty());
    }

    #[test]
    fn test_reconcile_blocks_removal_of_environments_with_topics() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        let env = env_repo::create_env(&conn, &repo_id, "old-env", "old-env", 0).unwrap();

        let topic = crate::db::topic_repo::create_topic(
            &conn,
            &repo_id,
            "feature/test",
            crate::types::BranchOrigin::Tracked,
            None,
            None,
        )
        .unwrap();
        crate::db::topic_env_repo::add_topic_to_env(&conn, &topic.id, &env.id).unwrap();

        let yaml = r#"version: "1"
environments: []"#;
        let config = crate::config::repo_config::parse_repo_config(yaml).unwrap();

        let summary = reconcile_environments(&conn, &repo_id, &config).unwrap();

        assert!(summary.added.is_empty());
        assert!(summary.removed.is_empty());
        assert_eq!(summary.blocked_removals.len(), 1);
        assert_eq!(summary.blocked_removals[0].env_name, "old-env");
        assert!(summary.blocked_removals[0].reason.contains("1 topics"));

        let envs = env_repo::list_envs(&conn, Some(&repo_id)).unwrap();
        assert_eq!(envs.len(), 1);
    }

    #[test]
    fn test_ordinal_from_position() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        let yaml = r#"version: "1"
environments:
  - staging
  - dev"#;
        let config = crate::config::repo_config::parse_repo_config(yaml).unwrap();

        reconcile_environments(&conn, &repo_id, &config).unwrap();

        let envs = env_repo::list_envs(&conn, Some(&repo_id)).unwrap();
        let staging = envs.iter().find(|e| e.name == "staging").unwrap();
        let dev = envs.iter().find(|e| e.name == "dev").unwrap();
        assert_eq!(staging.ordinal, 0);
        assert_eq!(dev.ordinal, 1);
    }

    #[test]
    fn test_update_env_changes_fields() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        let env = env_repo::create_env(&conn, &repo_id, "test", "test", 0).unwrap();

        env_repo::update_env(&conn, &env.id, "new-branch", 5).unwrap();

        let updated = env_repo::get_env(&conn, &env.id).unwrap();
        assert_eq!(updated.branch, "new-branch");
        assert_eq!(updated.ordinal, 5);
    }

    #[test]
    fn test_count_topics_in_env() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        let env = env_repo::create_env(&conn, &repo_id, "test", "test", 0).unwrap();

        assert_eq!(env_repo::count_topics_in_env(&conn, &env.id).unwrap(), 0);

        let topic1 = crate::db::topic_repo::create_topic(
            &conn,
            &repo_id,
            "feature/1",
            crate::types::BranchOrigin::Tracked,
            None,
            None,
        )
        .unwrap();
        let topic2 = crate::db::topic_repo::create_topic(
            &conn,
            &repo_id,
            "feature/2",
            crate::types::BranchOrigin::Tracked,
            None,
            None,
        )
        .unwrap();

        crate::db::topic_env_repo::add_topic_to_env(&conn, &topic1.id, &env.id).unwrap();
        crate::db::topic_env_repo::add_topic_to_env(&conn, &topic2.id, &env.id).unwrap();

        assert_eq!(env_repo::count_topics_in_env(&conn, &env.id).unwrap(), 2);
    }

    #[test]
    fn test_reconcile_empty_yaml_removes_all_envs_without_topics() {
        let (_workspace, conn) = setup_db();
        let repo_id = create_test_repo(&conn);

        env_repo::create_env(&conn, &repo_id, "dev", "dev", 0).unwrap();
        env_repo::create_env(&conn, &repo_id, "staging", "staging", 1).unwrap();

        let env_with_topic =
            env_repo::create_env(&conn, &repo_id, "old-env", "old-env", 2).unwrap();
        let topic = crate::db::topic_repo::create_topic(
            &conn,
            &repo_id,
            "feature/test",
            crate::types::BranchOrigin::Tracked,
            None,
            None,
        )
        .unwrap();
        crate::db::topic_env_repo::add_topic_to_env(&conn, &topic.id, &env_with_topic.id).unwrap();

        let yaml = r#"version: "1"
environments: []"#;
        let config = crate::config::repo_config::parse_repo_config(yaml).unwrap();

        let summary = reconcile_environments(&conn, &repo_id, &config).unwrap();

        assert_eq!(summary.removed.len(), 2);
        assert!(summary.removed.contains(&"dev".to_string()));
        assert!(summary.removed.contains(&"staging".to_string()));
        assert_eq!(summary.blocked_removals.len(), 1);
        assert_eq!(summary.blocked_removals[0].env_name, "old-env");

        let envs = env_repo::list_envs(&conn, Some(&repo_id)).unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "old-env");
    }
}
