use std::fmt;
use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum IdParseError {
    #[error("Invalid ULID format: {0}")]
    InvalidUlid(String),
    #[error("Missing prefix: expected '{expected}', got '{actual}'")]
    MissingPrefix {
        expected: &'static str,
        actual: String,
    },
}

fn validate_ulid(s: &str) -> Result<(), IdParseError> {
    ulid::Ulid::from_string(s)
        .map(|_| ())
        .map_err(|_| IdParseError::InvalidUlid(s.to_string()))
}

/// Generate a prefixed ULID newtype with all standard trait impls.
macro_rules! define_id {
    ($name:ident, $prefix:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;

            /// Generate new ID with fresh ULID
            pub fn new() -> Self {
                Self(format!("{}{}", Self::PREFIX, ulid::Ulid::new()))
            }

            /// Create from raw ULID (no prefix). Used by FromSql.
            pub(crate) fn from_raw_ulid(ulid: String) -> Self {
                Self(format!("{}{}", Self::PREFIX, ulid))
            }

            /// Extract the ULID part (without prefix)
            #[allow(dead_code)]
            pub fn ulid_part(&self) -> &str {
                self.0.strip_prefix(Self::PREFIX).unwrap_or(&self.0)
            }

            /// Full string representation (with prefix)
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let ulid = s
                    .strip_prefix(Self::PREFIX)
                    .ok_or_else(|| IdParseError::MissingPrefix {
                        expected: Self::PREFIX,
                        actual: s.to_string(),
                    })?;
                validate_ulid(ulid)?;
                Ok(Self::from_raw_ulid(ulid.to_string()))
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.0.clone()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                let s = value.as_str()?.to_string();
                Ok(Self(s))
            }
        }
    };
}

define_id!(RepoId, "repo_");
define_id!(TopicId, "topic_");
define_id!(EnvId, "env_");
define_id!(RebuildId, "rebuild_");
define_id!(ConflictId, "conflict_");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_id_new() {
        let id = RepoId::new();
        assert!(id.as_str().starts_with("repo_"));
        assert_eq!(id.ulid_part().len(), 26);
    }

    #[test]
    fn repo_id_parse_with_prefix() {
        let id: RepoId = "repo_01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap();
        assert_eq!(id.as_str(), "repo_01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn repo_id_parse_without_prefix_fails() {
        let result: Result<RepoId, _> = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse();
        assert!(matches!(result, Err(IdParseError::MissingPrefix { .. })));
    }

    #[test]
    fn repo_id_parse_invalid_ulid() {
        let result: Result<RepoId, _> = "repo_invalid".parse();
        assert!(matches!(result, Err(IdParseError::InvalidUlid(_))));
    }

    #[test]
    fn repo_id_serde() {
        let id = RepoId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert!(json.starts_with("\"repo_"));
        let parsed: RepoId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn topic_id_new() {
        let id = TopicId::new();
        assert!(id.as_str().starts_with("topic_"));
    }

    #[test]
    fn env_id_new() {
        let id = EnvId::new();
        assert!(id.as_str().starts_with("env_"));
    }

    #[test]
    fn rebuild_id_new() {
        let id = RebuildId::new();
        assert!(id.as_str().starts_with("rebuild_"));
    }

    #[test]
    fn conflict_id_new() {
        let id = ConflictId::new();
        assert!(id.as_str().starts_with("conflict_"));
    }
}
