use thiserror::Error;

use crate::id::{EnvId, RepoId, TopicId};

#[derive(Error, Debug)]
pub enum RestackError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(#[from] toml::de::Error),

    #[error("Git error: {0}")]
    Git(#[from] crate::git::GitError),

    #[error("Repo not found: {0}")]
    RepoNotFound(RepoId),

    #[error("Topic not found: {0}")]
    TopicNotFound(TopicId),

    #[error("Environment not found: {0}")]
    EnvNotFound(EnvId),

    #[error("Topic already tracked: {branch}")]
    TopicAlreadyTracked { branch: String },

    #[error("Topic '{topic}' is not in environment '{env}'")]
    TopicNotInEnv { topic: String, env: String },

    #[error("Merge conflict: topic '{topic}' conflicted with '{conflicted_with}'")]
    MergeConflict {
        topic: String,
        conflicted_with: String,
    },

    #[error("Rebuild failed for environment '{env}': {reason}")]
    RebuildFailed { env: String, reason: String },

    #[error("Not a restack workspace - run `restack init`")]
    NotAWorkspace,

    #[error("Not in a git repository")]
    NotInGitRepo,

    #[error("Invalid version: {version}")]
    InvalidVersion { version: String },

    #[error("No version tags found")]
    NoTagsFound,

    #[error("Maint merge conflict: {reason}")]
    MaintMergeConflict { reason: String },

    #[error("Provider not configured for this repository")]
    ProviderNotConfigured,

    #[error("Provider CLI not found: {0}")]
    ProviderCliNotFound(String),

    #[error("Provider API error: {0}")]
    ProviderApiError(String),

    #[error("Invalid ID: '{0}'")]
    InvalidId(String),

    #[error("Repo already tracked: {0}")]
    RepoAlreadyTracked(String),

    #[error("No repos tracked. Run `restack init` first.")]
    NoRepos,

    #[error("Multiple repos tracked. Specify --repo <id>.")]
    MultipleRepos,
}

pub type Result<T> = std::result::Result<T, RestackError>;
