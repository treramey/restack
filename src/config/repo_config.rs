use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{RestackError, Result};

/// Repository-level configuration from `.restack.yml` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoConfig {
    /// Schema version, must be "1"
    pub version: String,
    /// Environment entries in graduation order (index 0 = closest to production)
    #[serde(default)]
    pub environments: Vec<EnvironmentEntry>,
}

/// A single environment entry supporting two YAML forms:
/// - Simple string: `- dev` (name and branch are both "dev")
/// - Expanded object: `- name: staging\n  branch: release/staging`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum EnvironmentEntry {
    Simple(String),
    Full {
        name: String,
        #[serde(default)]
        branch: Option<String>,
    },
}

impl EnvironmentEntry {
    pub fn name(&self) -> &str {
        match self {
            EnvironmentEntry::Simple(s) => s,
            EnvironmentEntry::Full { name, .. } => name,
        }
    }

    pub fn branch(&self) -> String {
        match self {
            EnvironmentEntry::Simple(s) => s.clone(),
            EnvironmentEntry::Full { name, branch } => {
                branch.as_deref().unwrap_or(name).to_string()
            }
        }
    }
}

/// Default `.restack.yml` content for new repos.
pub const DEFAULT_RESTACK_YML: &str = r#"version: "1"

# Environment branches listed from development to production.
# Topics graduate through these environments in order.
#
# Simple form (branch name = environment name):
#   - staging
#
# Expanded form (when branch name differs from environment name):
#   - name: staging
#     branch: release/staging
environments:
  - dev
  - staging
"#;

/// Load repo config from a `.restack.yml` file.
pub fn load_repo_config(path: &Path) -> Result<RepoConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: RepoConfig = serde_yaml::from_str(&content).map_err(|e| {
        RestackError::RepoConfigValidation(format!(
            "Invalid .restack.yml\n\n  YAML syntax error: {}\n\n  Fix: Check YAML syntax at https://yaml-online-parser.appspot.com",
            e
        ))
    })?;
    Ok(config)
}

/// Parse repo config from a YAML string (useful for testing).
pub fn parse_repo_config(yaml: &str) -> Result<RepoConfig> {
    let config: RepoConfig = serde_yaml::from_str(yaml)?;
    Ok(config)
}

/// Validate that the version is "1".
pub fn validate_version(config: &RepoConfig) -> Result<()> {
    if config.version != "1" {
        return Err(RestackError::RepoConfigValidation(format!(
            "Invalid .restack.yml\n\n  version: \"{}\"\n          {}\n  Unsupported version '{}'. Supported: \"1\"\n\n  Fix: Change to version: \"1\"",
            config.version,
            "^".repeat(config.version.len()),
            config.version
        )));
    }
    Ok(())
}

/// Validate that no two environments share the same branch name.
pub fn validate_no_duplicate_branches(config: &RepoConfig) -> Result<()> {
    let mut seen_branches: HashMap<String, String> = HashMap::new();

    for entry in &config.environments {
        let env_name = entry.name().to_string();
        let branch = entry.branch();
        if let Some(existing_env) = seen_branches.get(&branch) {
            return Err(RestackError::RepoConfigValidation(format!(
                "Invalid .restack.yml\n\n  environments:\n    {}:\n      branch: {}\n    {}:\n      branch: {}\n\n  Duplicate branch '{}' used by both '{}' and '{}' environments.\n  Each environment must have a unique branch.\n\n  Fix: Give each environment a distinct branch name, or remove the 'branch:'\n       field to use the environment name as the branch.",
                existing_env, branch, env_name, branch, branch, existing_env, env_name
            )));
        }
        seen_branches.insert(branch, env_name);
    }

    Ok(())
}

/// Validate that no environment branch collides with the production branch.
pub fn validate_production_branch_collision(config: &RepoConfig, prod_branch: &str) -> Result<()> {
    for entry in &config.environments {
        let env_name = entry.name();
        let branch = entry.branch();
        if branch == prod_branch {
            return Err(RestackError::RepoConfigValidation(format!(
                "Invalid .restack.yml\n\n  environments:\n    {}:\n      branch: {}\n\n  Environment '{}' uses branch '{}' which is the production branch.\n  Integration environments must not use the production branch.\n\n  Fix: Change the branch to a different name (e.g., 'develop', 'staging'),\n       or remove the 'branch:' field to use the environment name as branch.",
                env_name, branch, env_name, branch
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string_entry() {
        let yaml = r#"version: "1"
environments:
  - dev"#;
        let config = parse_repo_config(yaml).unwrap();
        assert_eq!(config.version, "1");
        assert_eq!(config.environments.len(), 1);
        assert_eq!(config.environments[0].name(), "dev");
        assert_eq!(config.environments[0].branch(), "dev");
    }

    #[test]
    fn test_parse_full_entry() {
        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: develop
  - staging"#;
        let config = parse_repo_config(yaml).unwrap();

        assert_eq!(config.environments.len(), 2);

        assert_eq!(config.environments[0].name(), "dev");
        assert_eq!(config.environments[0].branch(), "develop");

        assert_eq!(config.environments[1].name(), "staging");
        assert_eq!(config.environments[1].branch(), "staging");
    }

    #[test]
    fn test_parse_full_entry_no_branch() {
        let yaml = r#"version: "1"
environments:
  - name: staging"#;
        let config = parse_repo_config(yaml).unwrap();
        assert_eq!(config.environments[0].name(), "staging");
        assert_eq!(config.environments[0].branch(), "staging");
    }

    #[test]
    fn test_parse_empty_environments() {
        let yaml = r#"version: "1""#;
        let config = parse_repo_config(yaml).unwrap();
        assert_eq!(config.version, "1");
        assert!(config.environments.is_empty());
    }

    #[test]
    fn test_parse_empty_list() {
        let yaml = r#"version: "1"
environments: []"#;
        let config = parse_repo_config(yaml).unwrap();
        assert!(config.environments.is_empty());
    }

    #[test]
    fn test_validate_version_valid() {
        let config = RepoConfig {
            version: "1".to_string(),
            environments: vec![],
        };
        assert!(validate_version(&config).is_ok());
    }

    #[test]
    fn test_validate_version_invalid() {
        let config = RepoConfig {
            version: "2".to_string(),
            environments: vec![],
        };
        let err = validate_version(&config).unwrap_err();
        assert!(err.to_string().contains("Invalid .restack.yml"));
        assert!(err.to_string().contains("Unsupported version '2'"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn test_validate_no_duplicate_branches_ok() {
        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: develop
  - staging"#;
        let config = parse_repo_config(yaml).unwrap();
        assert!(validate_no_duplicate_branches(&config).is_ok());
    }

    #[test]
    fn test_validate_no_duplicate_branches_with_simple_entries() {
        let yaml = r#"version: "1"
environments:
  - dev
  - staging"#;
        let config = parse_repo_config(yaml).unwrap();
        assert!(validate_no_duplicate_branches(&config).is_ok());
    }

    #[test]
    fn test_validate_no_duplicate_branches_detects_duplicate() {
        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: develop
  - name: develop
    branch: develop"#;
        let config = parse_repo_config(yaml).unwrap();
        let err = validate_no_duplicate_branches(&config).unwrap_err();
        assert!(err.to_string().contains("Duplicate branch 'develop'"));
    }

    #[test]
    fn test_validate_no_duplicate_branches_env_name_collision() {
        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: staging
  - staging"#;
        let config = parse_repo_config(yaml).unwrap();
        let err = validate_no_duplicate_branches(&config).unwrap_err();
        assert!(err.to_string().contains("Duplicate branch 'staging'"));
    }

    #[test]
    fn test_validate_production_branch_collision_ok() {
        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: develop
  - staging"#;
        let config = parse_repo_config(yaml).unwrap();
        assert!(validate_production_branch_collision(&config, "main").is_ok());
    }

    #[test]
    fn test_validate_production_branch_collision_detects_collision() {
        let yaml = r#"version: "1"
environments:
  - name: dev
    branch: main"#;
        let config = parse_repo_config(yaml).unwrap();
        let err = validate_production_branch_collision(&config, "main").unwrap_err();
        assert!(err
            .to_string()
            .contains("uses branch 'main' which is the production branch"));
    }

    #[test]
    fn test_ordinal_is_position() {
        let yaml = r#"version: "1"
environments:
  - staging
  - dev"#;
        let config = parse_repo_config(yaml).unwrap();
        // Index 0 = staging (closest to production)
        assert_eq!(config.environments[0].name(), "staging");
        // Index 1 = dev
        assert_eq!(config.environments[1].name(), "dev");
    }

    #[test]
    fn test_load_repo_config_file_not_found() {
        let result = load_repo_config(Path::new("/nonexistent/.restack.yml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let yaml = "version: [invalid";
        let result = parse_repo_config(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_config() {
        let config = RepoConfig {
            version: "1".to_string(),
            environments: vec![
                EnvironmentEntry::Full {
                    name: "dev".to_string(),
                    branch: Some("develop".to_string()),
                },
                EnvironmentEntry::Simple("staging".to_string()),
            ],
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("version"));
        assert!(yaml.contains("develop"));
    }

    #[test]
    fn test_default_restack_yml_parses() {
        let config = parse_repo_config(DEFAULT_RESTACK_YML).unwrap();
        assert_eq!(config.version, "1");
        assert_eq!(config.environments.len(), 2);
        assert_eq!(config.environments[0].name(), "dev");
        assert_eq!(config.environments[1].name(), "staging");
    }
}
