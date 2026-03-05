use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default = "WorkspaceSection::default")]
    pub workspace: WorkspaceSection,
    #[serde(default = "DefaultsSection::default")]
    pub defaults: DefaultsSection,
    #[serde(default)]
    pub environments: HashMap<String, EnvConfig>,
    #[serde(default = "RebuildSection::default")]
    pub rebuild: RebuildSection,
    #[serde(default = "ReleaseSection::default")]
    pub release: ReleaseSection,
    #[serde(default)]
    pub provider: ProviderSection,
    #[serde(default = "DiscoverySection::default")]
    pub discovery: DiscoverySection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSection {
    #[serde(default = "default_workspace_name")]
    pub name: String,
}

fn default_workspace_name() -> String {
    "restack-workspace".to_string()
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        Self {
            name: default_workspace_name(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsSection {
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_base_branch() -> String {
    "main".to_string()
}

fn default_provider() -> String {
    "github".to_string()
}

impl Default for DefaultsSection {
    fn default() -> Self {
        Self {
            base_branch: default_base_branch(),
            provider: default_provider(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    pub branch: String,
    #[serde(default)]
    pub ordinal: i32,
    #[serde(default)]
    pub auto_promote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildSection {
    #[serde(default = "default_force_push_mode")]
    pub force_push: String,
    #[serde(default = "default_true")]
    pub marker_commits: bool,
}

fn default_force_push_mode() -> String {
    "lease".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for RebuildSection {
    fn default() -> Self {
        Self {
            force_push: default_force_push_mode(),
            marker_commits: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSection {
    #[serde(default = "default_release_strategy")]
    pub strategy: String,
}

fn default_release_strategy() -> String {
    "merge".to_string()
}

impl Default for ReleaseSection {
    fn default() -> Self {
        Self {
            strategy: default_release_strategy(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSection {
    #[serde(default = "default_true")]
    pub auto_ci_refresh: bool,
    #[serde(default)]
    pub conflict_notifications: bool,
    #[serde(default)]
    pub github: GitHubConfig,
    #[serde(default)]
    pub azure: AzureConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub repo_slug: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AzureConfig {
    pub organization: Option<String>,
    pub project: Option<String>,
}

impl Default for ProviderSection {
    fn default() -> Self {
        Self {
            auto_ci_refresh: true,
            conflict_notifications: false,
            github: GitHubConfig::default(),
            azure: AzureConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverySection {
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,
    #[serde(default = "default_true")]
    pub exclude_env_branches: bool,
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "main".to_string(),
        "master".to_string(),
        "staging".to_string(),
        "dev".to_string(),
        "develop".to_string(),
        "production".to_string(),
        "maint".to_string(),
        "maint-*".to_string(),
        "release-*".to_string(),
        "hotfix-*".to_string(),
        "renovate/*".to_string(),
        "dependabot/*".to_string(),
    ]
}

impl Default for DiscoverySection {
    fn default() -> Self {
        Self {
            exclude_patterns: default_exclude_patterns(),
            exclude_env_branches: true,
        }
    }
}

impl DiscoverySection {
    pub fn should_exclude(&self, branch: &str, env_branches: &[&str]) -> bool {
        if self.exclude_env_branches && env_branches.iter().any(|e| *e == branch) {
            return true;
        }
        for pattern in &self.exclude_patterns {
            if Self::matches_pattern(branch, pattern) {
                return true;
            }
        }
        false
    }

    fn matches_pattern(branch: &str, pattern: &str) -> bool {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            branch.starts_with(prefix)
        } else if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            branch.ends_with(suffix)
        } else {
            branch == pattern
        }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        let mut environments = HashMap::new();
        environments.insert(
            "staging".to_string(),
            EnvConfig {
                branch: "staging".to_string(),
                ordinal: 0,
                auto_promote: false,
            },
        );
        environments.insert(
            "dev".to_string(),
            EnvConfig {
                branch: "dev".to_string(),
                ordinal: 1,
                auto_promote: true,
            },
        );

        Self {
            workspace: WorkspaceSection::default(),
            defaults: DefaultsSection::default(),
            environments,
            rebuild: RebuildSection::default(),
            release: ReleaseSection::default(),
            provider: ProviderSection::default(),
            discovery: DiscoverySection::default(),
        }
    }
}

pub fn load_config(path: &Path) -> Result<WorkspaceConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: WorkspaceConfig = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config(path: &Path, config: &WorkspaceConfig) -> Result<()> {
    let content = toml::to_string_pretty(config).map_err(std::io::Error::other)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn default_config() -> WorkspaceConfig {
    WorkspaceConfig::default()
}
