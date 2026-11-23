#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::cmp::Ordering;

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("invalid version {text:?}: it does not start with a number")]
    NoNumber { text: String },
    #[error("invalid version {text:?}: {part:?} is too large")]
    TooLarge { text: String, part: String },
    #[error("invalid version {text:?}: {part:?} is a wildcard, not a version")]
    Wildcard { text: String, part: String },
    #[error("invalid version {text:?}: {part:?} after a dot is not a number")]
    NotNumber { text: String, part: String },
    #[error("invalid version {text:?}: it ends in a separator")]
    Separator { text: String },
    #[error("invalid version {text:?}: {source}")]
    Suffix {
        text: String,
        #[source]
        source: semver::Error,
    },
}

#[derive(Debug, Clone)]
pub struct Version {
    core: semver::Version,
    extra: Vec<u64>,
    parts: usize,
}

impl Version {
    pub fn parse(text: &str) -> Result<Self, VersionError> {
        let trimmed = text.trim();
        let body = match trimmed.strip_prefix('v') {
            Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
            _ => trimmed,
        };
        let (body, build) = body.split_once('+').unwrap_or((body, ""));
        let mut numbers = Vec::new();
        let mut rest = body;
        loop {
            let tail = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            let number = rest.strip_suffix(tail).unwrap_or_default();
            if number.is_empty() {
                return Err(VersionError::NoNumber {
                    text: text.to_owned(),
                });
            }
            let value = number.parse().ok().ok_or_else(|| VersionError::TooLarge {
                text: text.to_owned(),
                part: number.to_owned(),
            })?;
            numbers.push(value);
            match tail.strip_prefix('.') {
                Some(next) if next.starts_with(|c: char| c.is_ascii_digit()) => rest = next,
                _ => {
                    rest = tail;
                    break;
                }
            }
        }
        if let Some(after) = rest.strip_prefix('.') {
            let part = after.split(['.', '-']).next().unwrap_or(after).to_owned();
            let text = text.to_owned();
            return Err(match part.as_str() {
                "" => VersionError::Separator { text },
                "x" | "X" | "*" => VersionError::Wildcard { text, part },
                _ => VersionError::NotNumber { text, part },
            });
        }
        let pre = rest.strip_prefix('-').unwrap_or(rest);
        if !rest.is_empty() && pre.is_empty() {
            return Err(VersionError::Separator {
                text: text.to_owned(),
            });
        }
        let suffix = |source| VersionError::Suffix {
            text: text.to_owned(),
            source,
        };
        let pre = semver::Prerelease::new(pre).map_err(suffix)?;
        let build = semver::BuildMetadata::new(build).map_err(suffix)?;
        let parts = numbers.len();
        let mut numbers = numbers.into_iter();
        let (major, minor, patch) = (
            numbers.next().unwrap_or(0),
            numbers.next().unwrap_or(0),
            numbers.next().unwrap_or(0),
        );
        Ok(Self {
            core: semver::Version {
                major,
                minor,
                patch,
                pre,
                build,
            },
            extra: numbers.collect(),
            parts,
        })
    }

    #[must_use]
    pub fn major(&self) -> u64 {
        self.core.major
    }

    #[must_use]
    pub fn minor(&self) -> u64 {
        self.core.minor
    }

    #[must_use]
    pub fn patch(&self) -> u64 {
        self.core.patch
    }

    #[must_use]
    pub fn prerelease(&self) -> &str {
        self.core.pre.as_str()
    }

    #[must_use]
    pub fn build(&self) -> &str {
        self.core.build.as_str()
    }

    #[must_use]
    pub fn is_prerelease(&self) -> bool {
        !self.core.pre.is_empty()
    }

    #[must_use]
    pub fn parts(&self) -> usize {
        self.parts
    }

    #[must_use]
    pub fn same_prefix(&self, other: &Self, count: usize) -> bool {
        self.numbers()
            .zip(other.numbers())
            .take(count)
            .all(|(left, right)| left == right)
    }

    fn numbers(&self) -> impl Iterator<Item = u64> + '_ {
        [self.core.major, self.core.minor, self.core.patch]
            .into_iter()
            .chain(self.extra.iter().copied())
            .chain(std::iter::repeat(0))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let width = 3 + self.extra.len().max(other.extra.len());
        self.numbers()
            .take(width)
            .cmp(other.numbers().take(width))
            .then_with(|| self.core.pre.cmp(&other.core.pre))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Version {}

impl std::str::FromStr for Version {
    type Err = VersionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

#[cfg(test)]
mod tests {
    use super::{Version, VersionError};

    fn parse(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn a_short_version_equals_its_zero_padded_form() {
        assert_eq!(parse("8.4"), parse("8.4.0"));
        assert_eq!(parse("8"), parse("8.0.0"));
        assert_eq!(parse("7.0.8.0"), parse("7.0.8"));
        assert_eq!(parse("v1.2.3"), parse("1.2.3"));
        assert_eq!(parse(" 9.0.1\n"), parse("9.0.1"));
        assert_eq!(parse("1.0.0+a"), parse("1.0.0+b"));
        assert_eq!(parse("8.4").parts(), 2);
        assert_eq!(parse("8.4.0").parts(), 3);
    }

    #[test]
    fn prereleases_sort_below_releases() {
        assert!(parse("8.4.0rc1") < parse("8.4.0"));
        assert!(parse("8.4.0rc1") < parse("8.4"));
        assert!(parse("8.4.0-rc1") < parse("8.4.0-rc2"));
        assert!(parse("8.4.0-pre.20250101.1") > parse("8.3.9"));
        assert_eq!(parse("8.4.0rc1"), parse("8.4.0-rc1"));
        assert_eq!(parse("8.4.0rc1").prerelease(), "rc1");
        assert!(parse("1.2.3-x").is_prerelease());
        assert!(parse("1.0.0-alpha.x") < parse("1.0.0"));
    }

    #[test]
    fn numeric_parts_compare_as_numbers() {
        assert!(parse("1.10.0") > parse("1.9.0"));
        assert!(parse("7.0.8.1") > parse("7.0.8"));
        assert!(parse("7.0.8.1") < parse("7.0.9"));
        assert!(parse("7.0.8.1.0") == parse("7.0.8.1"));
    }

    #[test]
    fn same_prefix_reads_missing_parts_as_zero() {
        assert!(parse("3.2.9").same_prefix(&parse("3.2.1"), 2));
        assert!(!parse("3.3.0").same_prefix(&parse("3.2.1"), 2));
        assert!(parse("3.0.5").same_prefix(&parse("3"), 2));
    }

    #[test]
    fn invalid_input_is_an_error() {
        for text in [
            "",
            "latest",
            "fork/8.4.0",
            "v",
            "1.2.",
            "1.2-",
            "8.x",
            "1.*",
            "1.5.0.post1",
            "5.6.15.Final",
        ] {
            assert!(Version::parse(text).is_err(), "{text:?}");
        }
        assert!(matches!(
            Version::parse("99999999999999999999"),
            Err(VersionError::TooLarge { .. })
        ));
        assert!(matches!(
            Version::parse("1.0.0-rc..1"),
            Err(VersionError::Suffix { .. })
        ));
        assert!(matches!(
            Version::parse("8.x"),
            Err(VersionError::Wildcard { .. })
        ));
        assert!(matches!(
            Version::parse("1.5.0.post1"),
            Err(VersionError::NotNumber { .. })
        ));
    }
}
