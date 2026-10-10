use super::sets::BumpSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WidenVersion {
    Semver(semver::Version),
    GitTag(String),
    GitCommit(String),
}

impl WidenVersion {
    pub fn display(&self) -> String {
        match self {
            WidenVersion::Semver(version) => version.to_string(),
            WidenVersion::GitTag(tag) => tag.clone(),
            WidenVersion::GitCommit(sha) => sha.clone(),
        }
    }

    pub fn is_semver(&self) -> bool {
        matches!(self, WidenVersion::Semver(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VersionError {
    #[error("empty version")]
    Empty,
    #[error(
        "invalid version {version:?} for set {set}: expected exact stable semver (e.g. 1.2.3)"
    )]
    InvalidSemver { set: &'static str, version: String },
    #[error("prerelease version {version:?} follows upstream resolver and project config, never a private dx policy")]
    Prerelease { version: String },
    #[error("invalid version {version:?} for set {set}: expected a Git tag (e.g. v4) or a 40/64-char commit SHA")]
    InvalidGit { set: &'static str, version: String },
}

pub fn is_stable(version: &semver::Version) -> bool {
    version.pre.is_empty()
}

pub fn compare(left: &semver::Version, right: &semver::Version) -> std::cmp::Ordering {
    left.cmp(right)
}

pub fn generic_major_bump_hint() -> &'static str {
    "if major bump, run `dx migrate --from <old> --to <new> --apply` (no manifest yet => migrate_failed exit 1; missing --from/--to => exit 2 missing-versions)"
}

pub fn parse(set: BumpSet, text: &str) -> Result<WidenVersion, VersionError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(VersionError::Empty);
    }
    match set {
        BumpSet::Bazel
        | BumpSet::Cargo
        | BumpSet::Npm
        | BumpSet::Go
        | BumpSet::Maven
        | BumpSet::NuGet => parse_semver(set.name(), trimmed),
        BumpSet::GithubActions => parse_git(set.name(), trimmed),
    }
}

fn parse_semver(set: &'static str, text: &str) -> Result<WidenVersion, VersionError> {
    let mut candidate = text.trim();
    candidate = candidate.strip_prefix("==").unwrap_or(candidate);
    candidate = candidate.strip_prefix('=').unwrap_or(candidate);
    candidate = candidate.strip_prefix('v').unwrap_or(candidate);
    candidate = candidate.trim();
    if candidate.is_empty() {
        return Err(VersionError::InvalidSemver {
            set,
            version: text.to_owned(),
        });
    }
    match candidate.parse::<semver::Version>() {
        Ok(version) => Ok(WidenVersion::Semver(version)),
        Err(_) => Err(VersionError::InvalidSemver {
            set,
            version: text.to_owned(),
        }),
    }
}

fn parse_git(set: &'static str, text: &str) -> Result<WidenVersion, VersionError> {
    let candidate = text.trim();
    if candidate.is_empty()
        || candidate.contains(' ')
        || candidate.contains(':')
        || candidate.contains('/')
    {
        return Err(VersionError::InvalidGit {
            set,
            version: text.to_owned(),
        });
    }
    if is_commit_sha(candidate) {
        return Ok(WidenVersion::GitCommit(candidate.to_owned()));
    }
    if candidate.len() <= 128 {
        return Ok(WidenVersion::GitTag(candidate.to_owned()));
    }
    Err(VersionError::InvalidGit {
        set,
        version: text.to_owned(),
    })
}

fn is_commit_sha(text: &str) -> bool {
    dx_digest::is_commit_sha(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_sets_require_exact_stable_shapes() {
        for set in [
            BumpSet::Bazel,
            BumpSet::Cargo,
            BumpSet::Npm,
            BumpSet::Go,
            BumpSet::Maven,
            BumpSet::NuGet,
        ] {
            let parsed = parse(set, "1.2.3").expect("semver");
            assert!(parsed.is_semver());
            assert_eq!(parsed.display(), "1.2.3");
            assert_eq!(parse(set, "v1.2.3").expect("v").display(), "1.2.3");
            assert_eq!(parse(set, "=1.2.3").expect("=").display(), "1.2.3");
            assert!(matches!(parse(set, ""), Err(VersionError::Empty)));
            assert!(matches!(
                parse(set, "not-a-version!!!"),
                Err(VersionError::InvalidSemver { .. })
            ));
            assert!(matches!(
                parse(set, "1.2"),
                Err(VersionError::InvalidSemver { .. })
            ));
        }
    }

    #[test]
    fn prerelease_parses_but_follows_upstream() {
        let parsed = parse(BumpSet::Cargo, "1.2.3-alpha.1").expect("prerelease parses");
        match parsed {
            WidenVersion::Semver(version) => {
                assert!(!is_stable(&version));
                assert_eq!(
                    compare(&version, &"1.2.3".parse().expect("stable")),
                    std::cmp::Ordering::Less
                );
            }
            _ => panic!("prerelease is semver"),
        }
        assert!(is_stable(&"1.2.3".parse().expect("stable")));
    }

    #[test]
    fn github_actions_accepts_tags_and_commits() {
        assert_eq!(
            parse(BumpSet::GithubActions, "v4").expect("tag").display(),
            "v4"
        );
        assert_eq!(
            parse(BumpSet::GithubActions, "v4.1.0")
                .expect("tag")
                .display(),
            "v4.1.0"
        );
        let sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
        assert_eq!(
            parse(BumpSet::GithubActions, sha).expect("sha"),
            WidenVersion::GitCommit(sha.to_owned())
        );
        assert!(matches!(
            parse(BumpSet::GithubActions, ""),
            Err(VersionError::Empty)
        ));
        assert!(matches!(
            parse(BumpSet::GithubActions, "bad tag"),
            Err(VersionError::InvalidGit { .. })
        ));
        assert!(matches!(
            parse(BumpSet::GithubActions, "owner/repo"),
            Err(VersionError::InvalidGit { .. })
        ));
    }

    #[test]
    fn github_actions_commit_sha_keeps_40_64_and_either_case() {
        let lower40 = "3d3c42e5aac5ba805825da76410c181273ba90b1";
        let upper40 = "3D3C42E5AAC5BA805825DA76410C181273BA90B1";
        for sha in [lower40, upper40] {
            assert_eq!(
                parse(BumpSet::GithubActions, sha).expect("40-char sha"),
                WidenVersion::GitCommit(sha.to_owned()),
                "40-char sha {sha:?} must stay a commit",
            );
        }
        let lower64 = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
        let upper64 = "9F86D081884C7D659A2FEAA0C55AD015A3BF4F1B2B0B822CD15D6C15B0F00A08";
        for sha in [lower64, upper64] {
            assert_eq!(
                parse(BumpSet::GithubActions, sha).expect("64-char sha"),
                WidenVersion::GitCommit(sha.to_owned()),
                "64-char sha {sha:?} must stay a commit",
            );
        }
        for bad in ["v4", "3d3c42e5", &"z".repeat(40), &"z".repeat(64)] {
            let parsed = parse(BumpSet::GithubActions, bad).expect("non-sha shape");
            assert!(
                matches!(parsed, WidenVersion::GitTag(_)),
                "{bad:?} must not parse as a commit"
            );
        }
        assert!(is_commit_sha(lower40));
        assert!(is_commit_sha(upper40));
    }

    #[test]
    fn ordering_delegates_to_upstream_semver() {
        let low: semver::Version = "1.2.3".parse().expect("low");
        let high: semver::Version = "1.10.0".parse().expect("high");
        assert_eq!(compare(&low, &high), std::cmp::Ordering::Less);
        assert_eq!(compare(&high, &low), std::cmp::Ordering::Greater);
        assert_eq!(compare(&low, &low), std::cmp::Ordering::Equal);
    }

    #[test]
    fn serde_json_and_toml_stay_upstream_owned() {
        let package: serde_json::Value =
            serde_json::from_str(r#"{"name":"react","version":"1.2.3"}"#).expect("json");
        assert_eq!(package["version"], serde_json::json!("1.2.3"));
        let manifest: toml::Table = toml::from_str("version = \"1.2.3\"\n").expect("toml");
        assert_eq!(manifest["version"].as_str(), Some("1.2.3"));
        let doc = "version = \"1.2.3\"\n"
            .parse::<toml_edit::DocumentMut>()
            .expect("toml_edit");
        assert_eq!(doc["version"].as_str(), Some("1.2.3"));
    }

    #[test]
    fn generic_major_bump_hint_pins_exit_mapping() {
        let generic = generic_major_bump_hint();
        assert!(generic.contains("dx migrate --from"), "{generic}");
        assert!(generic.contains("migrate_failed"), "{generic}");
        assert!(generic.contains("missing-versions"), "{generic}");
    }
}
