//! EasyTier's version: parsed from `--version` (`easytier-core 2.6.4-8428a89d`), a release tag (`v2.6.4`) or a
//! server's report, and compared against [`MIN_CORE_VERSION`], the one place the oldest supported release is named.

use std::{cmp::Ordering, fmt, str::FromStr};

/// The oldest EasyTier release the app drives. Contracts were checked against 2.6.4's binaries
/// (`tests/fixtures/easytier-2.6.4/`); 2.6.x share its config file, flags and JSON shapes.
pub const MIN_CORE_VERSION: CoreVersion = CoreVersion {
    major: 2,
    minor: 6,
    patch: 0,
    commit: None,
};

/// Why a version string did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    /// The text has no `major.minor.patch` triple.
    #[error("no version number in {0:?}")]
    Missing(String),
}

/// A release's number and, when the binary reports one, its commit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoreVersion {
    /// Major number.
    pub major: u32,
    /// Minor number.
    pub minor: u32,
    /// Patch number.
    pub patch: u32,
    /// The short commit after the number (`8428a89d`), if reported.
    pub commit: Option<String>,
}

impl CoreVersion {
    /// The version in a program's `--version` output (`easytier-core 2.6.4-8428a89d`): the first word that parses.
    ///
    /// # Errors
    ///
    /// [`VersionError::Missing`] when no word is a version.
    pub fn from_version_output(output: &str) -> Result<Self, VersionError> {
        output
            .split_whitespace()
            .find_map(|word| word.parse().ok())
            .ok_or_else(|| VersionError::Missing(output.chars().take(80).collect()))
    }

    /// Whether the app supports it.
    pub fn is_supported(&self) -> bool {
        *self >= MIN_CORE_VERSION
    }

    /// The release tag (`v2.6.4`).
    pub fn tag(&self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// The number without the commit (`2.6.4`): the name of its install directory.
    pub fn number(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for CoreVersion {
    type Err = VersionError;

    /// `2.6.4`, `v2.6.4`, `2.6.4-8428a89d`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let missing = || VersionError::Missing(text.chars().take(80).collect());
        let trimmed = text.trim().trim_start_matches(['v', 'V']);
        let (number, commit) = match trimmed.split_once(['-', '+']) {
            Some((number, rest)) => (number, Some(rest)),
            None => (trimmed, None),
        };
        let mut parts = number.split('.');
        let mut next = || -> Result<u32, VersionError> {
            parts
                .next()
                .and_then(|part| part.parse().ok())
                .ok_or_else(missing)
        };
        let (major, minor, patch) = (next()?, next()?, next()?);
        if parts.next().is_some() {
            return Err(missing());
        }
        let commit = commit
            .filter(|c| {
                !c.is_empty() && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.')
            })
            .map(str::to_owned);
        Ok(Self {
            major,
            minor,
            patch,
            commit,
        })
    }
}

impl PartialOrd for CoreVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CoreVersion {
    /// By number; the commit only breaks ties so `Ord` agrees with `Eq`.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch, &self.commit).cmp(&(
            other.major,
            other.minor,
            other.patch,
            &other.commit,
        ))
    }
}

impl fmt::Display for CoreVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        match &self.commit {
            Some(commit) => write!(f, "-{commit}"),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_line_of_each_program() {
        let core = include_str!("../../tests/fixtures/easytier-2.6.4/core-version.txt");
        let version = CoreVersion::from_version_output(core).unwrap();
        assert_eq!((version.major, version.minor, version.patch), (2, 6, 4));
        assert_eq!(version.commit.as_deref(), Some("8428a89d"));
        assert_eq!(version.to_string(), "2.6.4-8428a89d");
        assert!(version.is_supported());
    }

    #[test]
    fn reads_tags_and_rejects_garbage() {
        assert_eq!("v2.6.4".parse::<CoreVersion>().unwrap().number(), "2.6.4");
        assert!("2.6".parse::<CoreVersion>().is_err());
        assert!("2.6.4.1".parse::<CoreVersion>().is_err());
        assert!(CoreVersion::from_version_output("easytier-core").is_err());
    }

    #[test]
    fn older_releases_are_unsupported() {
        assert!(!"2.5.9".parse::<CoreVersion>().unwrap().is_supported());
        assert!("2.6.0".parse::<CoreVersion>().unwrap().is_supported());
        assert!("3.0.0".parse::<CoreVersion>().unwrap() > MIN_CORE_VERSION);
    }
}
