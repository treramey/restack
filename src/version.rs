use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::RestackError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(s: &str) -> crate::error::Result<Self> {
        let stripped = s.strip_prefix('v').unwrap_or(s);
        stripped.parse().map_err(|_| RestackError::InvalidVersion {
            version: s.to_string(),
        })
    }

    pub fn bump(self, bump_type: BumpType) -> Self {
        match bump_type {
            BumpType::Major => Self {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            },
            BumpType::Minor => Self {
                major: self.major,
                minor: self.minor + 1,
                patch: 0,
            },
            BumpType::Patch => Self {
                major: self.major,
                minor: self.minor,
                patch: self.patch + 1,
            },
        }
    }

    pub fn to_tag(self) -> String {
        format!("v{self}")
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for SemVer {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("expected X.Y.Z, got {s}"));
        }
        Ok(Self {
            major: parts[0]
                .parse()
                .map_err(|_| format!("invalid major: {}", parts[0]))?,
            minor: parts[1]
                .parse()
                .map_err(|_| format!("invalid minor: {}", parts[1]))?,
            patch: parts[2]
                .parse()
                .map_err(|_| format!("invalid patch: {}", parts[2]))?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BumpType {
    Major,
    Minor,
    Patch,
}

impl FromStr for BumpType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "major" => Ok(Self::Major),
            "minor" => Ok(Self::Minor),
            "patch" => Ok(Self::Patch),
            _ => Err(format!("invalid bump type: {s} (expected major, minor, or patch)")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v, SemVer { major: 1, minor: 2, patch: 3 });
    }

    #[test]
    fn parse_with_v_prefix() {
        let v = SemVer::parse("v0.10.5").unwrap();
        assert_eq!(v, SemVer { major: 0, minor: 10, patch: 5 });
    }

    #[test]
    fn parse_invalid() {
        assert!(SemVer::parse("not-a-version").is_err());
        assert!(SemVer::parse("1.2").is_err());
        assert!(SemVer::parse("v1.2.x").is_err());
    }

    #[test]
    fn bump_major() {
        let v = SemVer { major: 1, minor: 2, patch: 3 };
        assert_eq!(v.bump(BumpType::Major), SemVer { major: 2, minor: 0, patch: 0 });
    }

    #[test]
    fn bump_minor() {
        let v = SemVer { major: 1, minor: 2, patch: 3 };
        assert_eq!(v.bump(BumpType::Minor), SemVer { major: 1, minor: 3, patch: 0 });
    }

    #[test]
    fn bump_patch() {
        let v = SemVer { major: 1, minor: 2, patch: 3 };
        assert_eq!(v.bump(BumpType::Patch), SemVer { major: 1, minor: 2, patch: 4 });
    }

    #[test]
    fn to_tag() {
        let v = SemVer { major: 1, minor: 0, patch: 0 };
        assert_eq!(v.to_tag(), "v1.0.0");
    }

    #[test]
    fn display() {
        let v = SemVer { major: 3, minor: 14, patch: 159 };
        assert_eq!(format!("{v}"), "3.14.159");
    }

    #[test]
    fn bump_type_from_str() {
        assert_eq!("major".parse::<BumpType>().unwrap(), BumpType::Major);
        assert_eq!("Minor".parse::<BumpType>().unwrap(), BumpType::Minor);
        assert_eq!("PATCH".parse::<BumpType>().unwrap(), BumpType::Patch);
        assert!("invalid".parse::<BumpType>().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let v = SemVer { major: 1, minor: 2, patch: 3 };
        let json = serde_json::to_string(&v).unwrap();
        let parsed: SemVer = serde_json::from_str(&json).unwrap();
        assert_eq!(v, parsed);
    }
}
