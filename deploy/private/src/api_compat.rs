use std::fmt;

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::RunError;
use dx_semver::Version;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCompatMode {
    Off,
    Warn,
    Fail,
}

impl ApiCompatMode {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

#[must_use]
pub fn required_bump(previous: &str, next: &str) -> Option<Bump> {
    let old = Version::parse(previous).ok()?;
    let new = Version::parse(next).ok()?;
    let bump = if old.major() != new.major() || (old.major() == 0 && old.minor() != new.minor()) {
        Bump::Major
    } else if old.minor() != new.minor() {
        Bump::Minor
    } else {
        Bump::Patch
    };
    Some(bump)
}

#[must_use]
pub fn breaking_refusal(
    mode: ApiCompatMode,
    bump: Option<Bump>,
    breaking: usize,
) -> Option<String> {
    if mode != ApiCompatMode::Fail || breaking == 0 || bump == Some(Bump::Major) {
        return None;
    }
    Some(format!(
        "{breaking} breaking API change(s) need a major version bump"
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    Registry,
    Git,
}

impl Baseline {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "registry" => Some(Self::Registry),
            "git" => Some(Self::Git),
            _ => None,
        }
    }
}

pub fn highest_below<'a>(
    versions: impl IntoIterator<Item = &'a str>,
    next: &str,
) -> Option<String> {
    let next = Version::parse(next).ok()?;
    versions
        .into_iter()
        .map(str::trim)
        .filter_map(|version| {
            let parsed = Version::parse(version).ok()?;
            (!parsed.is_prerelease() && parsed < next).then(|| (parsed, version.to_owned()))
        })
        .max()
        .map(|(_, version)| version)
}

pub fn latest_published(index_body: &str, next: &str) -> Option<String> {
    let versions: Vec<String> = index_body
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|entry| !entry["yanked"].as_bool().unwrap_or(false))
        .filter_map(|entry| {
            entry
                .get("vers")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    highest_below(versions.iter().map(String::as_str), next)
}

#[must_use]
pub fn previous_tag(tags: &str, next: &str) -> Option<String> {
    highest_below(
        tags.lines().filter_map(|tag| tag.trim().strip_prefix('v')),
        next,
    )
    .map(|version| format!("v{version}"))
}

#[must_use]
pub fn previous_release(metadata: &str, next: &str) -> Option<String> {
    let versions = metadata
        .split("<version>")
        .skip(1)
        .filter_map(|part| part.split("</version>").next());
    highest_below(versions, next)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    FirstRelease,
    Skipped(String),
    Compared {
        previous: String,
        changes: Vec<String>,
    },
}

#[derive(Debug)]
pub enum ApiCompatError {
    Breaking {
        rule: String,
        reason: String,
        previous: String,
        next: String,
    },
    Client {
        source: dx_http::Error,
    },
    Query {
        url: String,
        source: dx_http::Error,
    },
    Git {
        source: RunError,
    },
    Export {
        source: PathIoError,
    },
}

impl fmt::Display for ApiCompatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Breaking {
                rule,
                reason,
                previous,
                next,
            } => write!(f, "{rule}: api compat: {reason} ({previous} -> {next})"),
            Self::Client { source } => write!(f, "{source}"),
            Self::Query { url, source } => write!(f, "cannot query {url}: {source}"),
            Self::Git { source } => write!(f, "{source}"),
            Self::Export { source } => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for ApiCompatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Breaking { .. } => None,
            Self::Client { source } | Self::Query { source, .. } => Some(source),
            Self::Git { source } => Some(source),
            Self::Export { source } => Some(source),
        }
    }
}

const REPORT_LINES: usize = 40;

pub fn conclude(
    rule: &str,
    mode: ApiCompatMode,
    next: &str,
    outcome: Outcome,
) -> Result<(), ApiCompatError> {
    if mode == ApiCompatMode::Off {
        return Ok(());
    }
    let (previous, changes) = match outcome {
        Outcome::FirstRelease => {
            println!("api compat: no earlier release; passed (first release)");
            return Ok(());
        }
        Outcome::Skipped(why) => {
            println!("api compat: skipped: {why}");
            return Ok(());
        }
        Outcome::Compared { previous, changes } => (previous, changes),
    };
    let count = changes.len();
    println!("api compat: {count} breaking change(s) against {previous} ({previous} -> {next})");
    for change in changes.iter().take(REPORT_LINES) {
        println!("api compat: - {change}");
    }
    if count > REPORT_LINES {
        println!("api compat: - ... and {} more", count - REPORT_LINES);
    }
    let bump = required_bump(
        previous.trim_start_matches('v'),
        next.trim_start_matches('v'),
    );
    match breaking_refusal(mode, bump, count) {
        Some(reason) => Err(ApiCompatError::Breaking {
            rule: rule.to_owned(),
            reason,
            previous,
            next: next.to_owned(),
        }),
        None => Ok(()),
    }
}

pub fn changes_or_failure(
    changes: Vec<String>,
    success: bool,
    tool: &str,
    output: &str,
) -> Vec<String> {
    if success || !changes.is_empty() {
        return changes;
    }
    let tail: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let tail = tail
        .iter()
        .skip(tail.len().saturating_sub(5))
        .copied()
        .collect::<Vec<_>>()
        .join(" | ");
    vec![format!("{tool} failed: {tail}")]
}

#[must_use]
pub fn workspace_dir() -> std::path::PathBuf {
    std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .unwrap_or_else(|_| ".".to_owned())
        .into()
}

pub fn previous_git_tag(
    workspace: &std::path::Path,
    next: &str,
) -> Result<Option<String>, ApiCompatError> {
    let output = dx_process::capture::run_checked(
        dx_process::capture::git(workspace).args(["tag", "--list", "v*"]),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .map_err(|source| ApiCompatError::Git { source })?;
    Ok(previous_tag(&output.stdout_text(), next))
}

pub fn fetch(url: &str, limit: u64) -> Result<Option<Vec<u8>>, ApiCompatError> {
    let client = crate::http::client().map_err(|source| ApiCompatError::Client { source })?;
    fetch_with(&client, url, limit)
}

pub fn fetch_with(
    client: &dx_http::Client,
    url: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, ApiCompatError> {
    match client.bytes(url, &[], limit) {
        Ok(body) => Ok(Some(body)),
        Err(dx_http::Error::Status {
            code: 404 | 410, ..
        }) => Ok(None),
        Err(source) => Err(ApiCompatError::Query {
            url: url.to_owned(),
            source,
        }),
    }
}

pub fn fetch_to(url: &str, dest: &std::path::Path, limit: u64) -> Result<bool, ApiCompatError> {
    let client = crate::http::client().map_err(|source| ApiCompatError::Client { source })?;
    fetch_to_with(&client, url, dest, limit)
}

pub fn fetch_to_with(
    client: &dx_http::Client,
    url: &str,
    dest: &std::path::Path,
    limit: u64,
) -> Result<bool, ApiCompatError> {
    match client.download_to(url, &[], dest, limit) {
        Ok(_) => Ok(true),
        Err(dx_http::Error::Status {
            code: 404 | 410, ..
        }) => Ok(false),
        Err(source) => Err(ApiCompatError::Query {
            url: url.to_owned(),
            source,
        }),
    }
}

pub fn export_git_dir(
    workspace: &std::path::Path,
    rel_dir: &str,
    rev: &str,
    dest: &std::path::Path,
    marker: &str,
) -> Result<bool, ApiCompatError> {
    let git = |args: &[&str]| -> Result<Vec<u8>, ApiCompatError> {
        dx_process::capture::run_checked(
            dx_process::capture::git(workspace).args(args),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .map(|output| output.stdout)
        .map_err(|source| ApiCompatError::Git { source })
    };
    let dir = rel_dir.trim_matches('/');
    let spec = if dir.is_empty() { "." } else { dir };
    let listing = git(&["ls-tree", "-r", "-z", "--full-name", rev, "--", spec])?;
    let mut found = false;
    for entry in listing.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let mut fields = meta.split(' ');
        let (Some(mode), Some(_), Some(oid)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        let rel = if dir.is_empty() {
            path
        } else {
            match path.strip_prefix(dir).and_then(|r| r.strip_prefix('/')) {
                Some(rel) => rel,
                None => continue,
            }
        };
        if mode == "120000" || mode == "160000" {
            println!("api compat: skipped {path} at {rev} (symlink or submodule)");
            continue;
        }
        let blob = git(&["cat-file", "blob", oid])?;
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            path_io::create_dir_all(parent).map_err(|source| ApiCompatError::Export { source })?;
        }
        path_io::write_scratch(&target, &blob)
            .map_err(|source| ApiCompatError::Export { source })?;
        found |= rel == marker;
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_ecosystems::Ecosystem;

    #[test]
    fn bump_follows_semver() {
        assert_eq!(required_bump("1.2.0", "1.2.1"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0", "1.3.0"), Some(Bump::Minor));
        assert_eq!(required_bump("1.2.0", "2.0.0"), Some(Bump::Major));
        assert_eq!(required_bump("0.3.1", "0.4.0"), Some(Bump::Major));
        assert_eq!(required_bump("0.3.1", "0.3.2"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0", "x"), None);
        assert_eq!(required_bump("1.2", "1.2.1"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0-rc.1", "1.2.0"), Some(Bump::Patch));
    }

    #[test]
    fn breaking_fails_unless_major() {
        let fail = ApiCompatMode::Fail;
        assert!(breaking_refusal(fail, Some(Bump::Patch), 1).is_some());
        assert!(breaking_refusal(fail, Some(Bump::Minor), 2).is_some());
        assert!(breaking_refusal(fail, None, 1).is_some());
        assert_eq!(breaking_refusal(fail, Some(Bump::Major), 3), None);
        assert_eq!(breaking_refusal(fail, Some(Bump::Patch), 0), None);
        assert_eq!(
            breaking_refusal(ApiCompatMode::Warn, Some(Bump::Patch), 1),
            None
        );
        assert_eq!(
            breaking_refusal(ApiCompatMode::Off, Some(Bump::Patch), 1),
            None
        );
    }

    #[test]
    fn latest_published_skips_yanked_and_prerelease() {
        let body = "{\"vers\":\"1.0.0\",\"yanked\":false}\n{\"vers\":\"1.10.0\",\"yanked\":false}\n{\"vers\":\"1.11.0\",\"yanked\":true}\n{\"vers\":\"2.0.0-rc.1\",\"yanked\":false}\n";
        assert_eq!(latest_published(body, "3.0.0").as_deref(), Some("1.10.0"));
        assert_eq!(latest_published(body, "1.5.0").as_deref(), Some("1.0.0"));
        assert_eq!(latest_published("", "1.0.0"), None);
    }

    #[test]
    fn previous_tag_picks_highest_below_next() {
        let tags = "v1.2.0\nv1.10.0\nv2.0.0\nv1.11.0-rc.1\nnightly\n";
        assert_eq!(previous_tag(tags, "2.0.0").as_deref(), Some("v1.10.0"));
        assert_eq!(previous_tag(tags, "v2.0.1").as_deref(), Some("v2.0.0"));
        assert_eq!(previous_tag(tags, "1.0.0"), None);
    }

    #[test]
    fn previous_release_picks_highest_below_next() {
        let xml = "<versions><version>1.2.0</version><version>1.10.0</version><version>2.0.0</version><version>2.1.0-SNAPSHOT</version></versions>";
        assert_eq!(previous_release(xml, "2.0.0").as_deref(), Some("1.10.0"));
        assert_eq!(previous_release(xml, "3.0.0").as_deref(), Some("2.0.0"));
        assert_eq!(previous_release(xml, "1.0.0"), None);
        assert_eq!(previous_release("", "1.0.0"), None);
    }

    #[test]
    fn baseline_parses_registry_and_git() {
        assert_eq!(Baseline::parse("registry"), Some(Baseline::Registry));
        assert_eq!(Baseline::parse("git"), Some(Baseline::Git));
        assert_eq!(Baseline::parse("tag"), None);
    }

    #[test]
    fn highest_below_skips_prerelease_and_higher() {
        let versions = ["0.9.0", "1.4.2", "1.10.0", "2.0.0-rc.1", "2.0.0", "junk"];
        assert_eq!(highest_below(versions, "2.0.0").as_deref(), Some("1.10.0"));
        assert_eq!(highest_below(versions, "v2.1.0").as_deref(), Some("2.0.0"));
        assert_eq!(highest_below(versions, "0.9.0"), None);
        assert_eq!(highest_below(versions, "next"), None);
        let short = ["1.4", "1.5.0rc1", "1.5.0"];
        assert_eq!(highest_below(short, "1.5.0").as_deref(), Some("1.4"));
        assert_eq!(highest_below(short, "1.5.1").as_deref(), Some("1.5.0"));
        assert_eq!(highest_below(["1.4.0", "1.5.0"], "1.5.0.post1"), None);
        assert_eq!(
            highest_below(["1.4.0", "1.5.0.post1", "1.5.0"], "1.6.0").as_deref(),
            Some("1.5.0")
        );
    }

    fn compared(previous: &str, changes: &[&str]) -> Outcome {
        Outcome::Compared {
            previous: previous.to_owned(),
            changes: changes.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    #[test]
    fn conclude_passes_a_first_release_and_a_skip() {
        let fail = ApiCompatMode::Fail;
        assert!(matches!(
            conclude("x_deploy", fail, "1.0.0", Outcome::FirstRelease),
            Ok(())
        ));
        let skipped = Outcome::Skipped("registry unreachable".to_owned());
        assert!(matches!(
            conclude("x_deploy", fail, "1.0.0", skipped),
            Ok(())
        ));
    }

    #[test]
    fn conclude_refuses_breaking_changes_without_a_major_bump() {
        let refused = conclude(
            "x_deploy",
            ApiCompatMode::Fail,
            "1.3.0",
            compared("1.2.0", &["gone"]),
        );
        assert!(matches!(refused, Err(ApiCompatError::Breaking { .. })));
        assert_eq!(
            refused.expect_err("refused").to_string(),
            "x_deploy: api compat: 1 breaking API change(s) need a major version bump (1.2.0 -> 1.3.0)"
        );
        let tags = conclude(
            "x_deploy",
            ApiCompatMode::Fail,
            "v1.3.0",
            compared("v1.2.0", &["a", "b"]),
        );
        let tags = tags.expect_err("refused").to_string();
        assert!(tags.contains("2 breaking API change(s)") && tags.contains("(v1.2.0 -> v1.3.0)"));
    }

    #[test]
    fn conclude_allows_a_major_bump_warn_off_and_no_changes() {
        let gone = || compared("1.2.0", &["gone"]);
        assert!(matches!(
            conclude("x_deploy", ApiCompatMode::Fail, "2.0.0", gone()),
            Ok(())
        ));
        assert!(matches!(
            conclude("x_deploy", ApiCompatMode::Warn, "1.3.0", gone()),
            Ok(())
        ));
        assert!(matches!(
            conclude("x_deploy", ApiCompatMode::Off, "1.3.0", gone()),
            Ok(())
        ));
        assert!(matches!(
            conclude(
                "x_deploy",
                ApiCompatMode::Fail,
                "1.2.1",
                compared("1.2.0", &[])
            ),
            Ok(())
        ));
    }

    #[test]
    fn changes_or_failure_reports_a_tool_failure_without_changes() {
        assert_eq!(
            changes_or_failure(vec![], true, "t", "x"),
            Vec::<String>::new()
        );
        assert_eq!(
            changes_or_failure(vec!["a".to_owned()], false, "t", "x"),
            vec!["a"]
        );
        assert_eq!(
            changes_or_failure(vec![], false, "t", "\nboom\n"),
            vec!["t failed: boom"]
        );
    }

    #[test]
    fn export_git_dir_writes_only_the_directory() {
        let root = std::env::temp_dir().join(format!("export-git-dir-{}", std::process::id()));
        dx_atomic_fs::discard_dir(&root);
        let repo = root.join("repo");
        std::fs::create_dir_all(repo.join("pkg/src")).expect("dirs");
        std::fs::create_dir_all(repo.join("other")).expect("dirs");
        std::fs::write(
            repo.join("pkg").join(Ecosystem::Cargo.manifest_name()),
            "[package]\n",
        )
        .expect("write");
        std::fs::write(repo.join("pkg/src/lib.rs"), "pub fn a() {}\n").expect("write");
        std::fs::write(repo.join("other/x.txt"), "x").expect("write");
        std::fs::write(
            repo.join("other").join(Ecosystem::Pub.manifest_name()),
            "name: x\n",
        )
        .expect("write");
        dx_test_scratch::git(&repo, &repo, &["init", "-q"]).unwrap();
        dx_test_scratch::git(&repo, &repo, &["add", "."]).unwrap();
        dx_test_scratch::git(&repo, &repo, &["commit", "-q", "-m", "x"]).unwrap();
        dx_test_scratch::git(&repo, &repo, &["tag", "v1.0.0"]).unwrap();
        let dest = root.join("dest");
        assert!(
            export_git_dir(
                &repo,
                "pkg",
                "v1.0.0",
                &dest,
                Ecosystem::Cargo.manifest_name()
            )
            .expect("export")
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("src/lib.rs")).expect("lib"),
            "pub fn a() {}\n"
        );
        assert!(dest.join(Ecosystem::Cargo.manifest_name()).exists());
        assert!(!dest.join("other").exists() && !dest.join("x.txt").exists());
        let missing = root.join("missing");
        assert!(
            !export_git_dir(
                &repo,
                "other",
                "v1.0.0",
                &missing,
                Ecosystem::Cargo.manifest_name()
            )
            .expect("export")
        );
        assert!(
            export_git_dir(
                &repo,
                "other",
                "v1.0.0",
                &root.join("pub"),
                Ecosystem::Pub.manifest_name()
            )
            .expect("export")
        );
        assert!(
            !export_git_dir(
                &repo,
                "nope",
                "v1.0.0",
                &missing,
                Ecosystem::Cargo.manifest_name()
            )
            .expect("export")
        );
        let status = dx_process::capture::run_checked(
            dx_process::capture::git(&repo).args(["status", "--porcelain"]),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .expect("status");
        assert!(status.stdout.is_empty(), "{:?}", status.stdout);
        dx_atomic_fs::discard_dir(&root);
    }

    #[test]
    fn export_git_dir_reports_an_unwritable_dest() {
        let root = std::env::temp_dir().join(format!("export-git-dir-file-{}", std::process::id()));
        dx_atomic_fs::discard_dir(&root);
        let repo = root.join("repo");
        std::fs::create_dir_all(repo.join("pkg/src")).expect("dirs");
        std::fs::write(repo.join("pkg/src/lib.rs"), "pub fn a() {}\n").expect("write");
        dx_test_scratch::git(&repo, &repo, &["init", "-q"]).unwrap();
        dx_test_scratch::git(&repo, &repo, &["add", "."]).unwrap();
        dx_test_scratch::git(&repo, &repo, &["commit", "-q", "-m", "x"]).unwrap();
        let blocker = root.join("blocker");
        std::fs::write(&blocker, "x").expect("write");
        let error = export_git_dir(
            &repo,
            "pkg",
            "HEAD",
            &blocker.join("dest"),
            Ecosystem::Cargo.manifest_name(),
        )
        .expect_err("dest under a file");
        assert!(matches!(error, ApiCompatError::Export { .. }), "{error:?}");
        assert!(error.to_string().starts_with("cannot create "), "{error}");
        dx_atomic_fs::discard_dir(&root);
    }
}
