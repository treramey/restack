use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{ConflictId, EnvId, RebuildId, RepoId, TopicId};
use crate::version::BumpType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    GitHub,
    AzureDevOps,
    Bitbucket,
    Unknown,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::AzureDevOps => write!(f, "azure"),
            Self::Bitbucket => write!(f, "bitbucket"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TopicStatus {
    Active,
    Conflict,
    Graduated,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CiStatus {
    Pending,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RebuildStatus {
    Running,
    Success,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForcePushMode {
    Lease,
    Never,
    Force,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub id: RepoId,
    pub name: String,
    pub path: String,
    pub remote_url: Option<String>,
    pub provider: Provider,
    pub base_branch: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: EnvId,
    pub repo_id: RepoId,
    pub name: String,
    pub branch: String,
    pub ordinal: i32,
    pub auto_promote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub id: TopicId,
    pub repo_id: RepoId,
    pub branch: String,
    pub pr_id: Option<String>,
    pub pr_url: Option<String>,
    pub status: TopicStatus,
    pub ci_status: Option<CiStatus>,
    pub ci_url: Option<String>,
    pub last_ci_check: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicEnvironment {
    pub topic_id: TopicId,
    pub env_id: EnvId,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rebuild {
    pub id: RebuildId,
    pub env_id: EnvId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: RebuildStatus,
    pub topics_merged: i32,
    pub topics_conflicted: i32,
    pub result_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: ConflictId,
    pub rebuild_id: RebuildId,
    pub topic_id: TopicId,
    pub conflicted_with: Option<String>,
    pub resolved: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Release / Hotfix types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConventionalCommit {
    #[serde(rename = "type")]
    pub commit_type: String,
    pub scope: Option<String>,
    pub breaking: bool,
    pub description: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogSection {
    pub title: String,
    pub entries: Vec<ChangelogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub description: String,
    pub sha: String,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub tag: String,
    pub bump_type: BumpType,
    pub changelog: Vec<ChangelogSection>,
    pub previous_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotfixInfo {
    pub version: String,
    pub tag: String,
    pub maint_branch: String,
    pub merged_to_master: bool,
}
