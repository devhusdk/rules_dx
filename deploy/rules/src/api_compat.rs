//! Public API compatibility rule shared by the registry adapters.

/// What `api_compat` does with a breaking report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCompatMode {
    Off,
    Warn,
    Fail,
}

impl ApiCompatMode {
    /// Parses the `api_compat` attribute value.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

/// How large a version bump is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

/// Returns the numeric `major.minor.patch` of a version; build and pre-release parts are ignored.
pub fn parse_version(version: &str) -> Option<[u64; 3]> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let mut out = [0u64; 3];
    for slot in &mut out {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

/// Returns the bump from `previous` to `next`; None when either is not a plain version.
/// Below 1.0.0 a minor change counts as major, as cargo treats it.
pub fn required_bump(previous: &str, next: &str) -> Option<Bump> {
    let (old, new) = (parse_version(previous)?, parse_version(next)?);
    let bump = if old[0] != new[0] || (old[0] == 0 && old[1] != new[1]) {
        Bump::Major
    } else if old[1] != new[1] {
        Bump::Minor
    } else {
        Bump::Patch
    };
    Some(bump)
}

/// Returns an error when a breaking report is not allowed by the version bump.
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

/// Where the earlier version an `api_compat` check compares against comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    /// The highest published release below the new version.
    Registry,
    /// The highest `v*` git tag below the new version.
    Git,
}

impl Baseline {
    /// Parses the `api_baseline` attribute value; empty means `registry`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "" | "registry" => Some(Self::Registry),
            "git" => Some(Self::Git),
            _ => None,
        }
    }
}

/// Returns the highest non-prerelease version below `next`.
pub fn highest_below<'a>(
    versions: impl IntoIterator<Item = &'a str>,
    next: &str,
) -> Option<String> {
    let next = parse_version(next.trim_start_matches('v'))?;
    versions
        .into_iter()
        .map(str::trim)
        .filter(|version| !version.contains('-'))
        .filter_map(|version| {
            let parsed = parse_version(version)?;
            (parsed < next).then(|| (parsed, version.to_owned()))
        })
        .max()
        .map(|(_, version)| version)
}

/// Returns the highest non-yanked release below `next` in a sparse index body.
pub fn latest_published(index_body: &str, next: &str) -> Option<String> {
    let versions: Vec<String> = index_body
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|entry| !entry["yanked"].as_bool().unwrap_or(false))
        .filter_map(|entry| entry["vers"].as_str().map(str::to_owned))
        .collect();
    highest_below(versions.iter().map(String::as_str), next)
}

/// Returns the highest `v<version>` tag below `next` from `git tag` output.
pub fn previous_tag(tags: &str, next: &str) -> Option<String> {
    highest_below(
        tags.lines().filter_map(|tag| tag.trim().strip_prefix('v')),
        next,
    )
    .map(|version| format!("v{version}"))
}

/// Returns the highest release below `next` listed in a `maven-metadata.xml` body.
pub fn previous_release(metadata: &str, next: &str) -> Option<String> {
    let versions = metadata
        .split("<version>")
        .skip(1)
        .filter_map(|part| part.split("</version>").next());
    highest_below(versions, next)
}

/// What an adapter found when it compared the new API with the baseline.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// There is no earlier version to compare with.
    FirstRelease,
    /// The check could not run; the reason is printed and the deploy goes on.
    Skipped(String),
    /// The tool ran against `previous`; `changes` lists each breaking change.
    Compared {
        previous: String,
        changes: Vec<String>,
    },
}

/// How many breaking changes the report prints.
const REPORT_LINES: usize = 40;

/// Prints the report of an `api_compat` check and refuses a breaking change without a major bump.
pub fn conclude(
    rule: &str,
    mode: ApiCompatMode,
    next: &str,
    outcome: Outcome,
) -> Result<(), String> {
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
        Some(reason) => Err(format!(
            "{rule}: api compat: {reason} ({previous} -> {next})"
        )),
        None => Ok(()),
    }
}

/// Returns `changes`, or the last lines of `output` when a failed tool listed none.
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
    let tail = tail[tail.len().saturating_sub(5)..].join(" | ");
    vec![format!("{tool} failed: {tail}")]
}

/// The repository a deploy runs from.
pub fn workspace_dir() -> std::path::PathBuf {
    std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .unwrap_or_else(|_| ".".to_owned())
        .into()
}

/// Returns the highest `v<version>` tag below `next` in the repository at `workspace`.
pub fn previous_git_tag(workspace: &std::path::Path, next: &str) -> Result<Option<String>, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["tag", "--list", "v*"])
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !output.status.success() {
        return Err("git tag --list failed".to_owned());
    }
    Ok(previous_tag(&String::from_utf8_lossy(&output.stdout), next))
}

/// Downloads `url` up to `limit` bytes; None when the server answers 404 or 410.
pub fn fetch(url: &str, limit: u64) -> Result<Option<Vec<u8>>, String> {
    fetch_with(&crate::http::publish_agent(), url, limit)
}

/// Downloads `url` with `agent` up to `limit` bytes; None when the server answers 404 or 410.
pub fn fetch_with(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Option<Vec<u8>>, String> {
    match agent.get(url).call() {
        Ok(response) if matches!(response.status().as_u16(), 404 | 410) => Ok(None),
        Ok(response) if !response.status().is_success() => {
            Err(format!("cannot query {url}: HTTP {}", response.status()))
        }
        Ok(mut response) => response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map(Some)
            .map_err(|error| format!("cannot read {url}: {error}")),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(error) => Err(format!("cannot query {url}: {error}")),
    }
}

/// Writes the files under `rel_dir` at `rev` into `dest`; false when that directory has no `marker` file.
/// Symlinks and submodules are skipped. The index and working tree are not touched.
pub fn export_git_dir(
    workspace: &std::path::Path,
    rel_dir: &str,
    rev: &str,
    dest: &std::path::Path,
    marker: &str,
) -> Result<bool, String> {
    let git = |args: &[&str]| -> Result<Vec<u8>, String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(args)
            .output()
            .map_err(|error| format!("git: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(output.stdout)
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
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(&target, blob).map_err(|error| format!("{}: {error}", target.display()))?;
        found |= rel == marker;
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_follows_semver() {
        assert_eq!(required_bump("1.2.0", "1.2.1"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0", "1.3.0"), Some(Bump::Minor));
        assert_eq!(required_bump("1.2.0", "2.0.0"), Some(Bump::Major));
        assert_eq!(required_bump("0.3.1", "0.4.0"), Some(Bump::Major));
        assert_eq!(required_bump("0.3.1", "0.3.2"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0", "x"), None);
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
        assert_eq!(Baseline::parse(""), Some(Baseline::Registry));
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
        assert_eq!(
            conclude("x_deploy", fail, "1.0.0", Outcome::FirstRelease),
            Ok(())
        );
        let skipped = Outcome::Skipped("registry unreachable".to_owned());
        assert_eq!(conclude("x_deploy", fail, "1.0.0", skipped), Ok(()));
    }

    #[test]
    fn conclude_refuses_breaking_changes_without_a_major_bump() {
        let refused = conclude(
            "x_deploy",
            ApiCompatMode::Fail,
            "1.3.0",
            compared("1.2.0", &["gone"]),
        );
        assert_eq!(
            refused,
            Err("x_deploy: api compat: 1 breaking API change(s) need a major version bump (1.2.0 -> 1.3.0)".to_owned())
        );
        let tags = conclude(
            "x_deploy",
            ApiCompatMode::Fail,
            "v1.3.0",
            compared("v1.2.0", &["a", "b"]),
        );
        assert!(tags.is_err_and(
            |e| e.contains("2 breaking API change(s)") && e.contains("(v1.2.0 -> v1.3.0)")
        ));
    }

    #[test]
    fn conclude_allows_a_major_bump_warn_off_and_no_changes() {
        let gone = || compared("1.2.0", &["gone"]);
        assert_eq!(
            conclude("x_deploy", ApiCompatMode::Fail, "2.0.0", gone()),
            Ok(())
        );
        assert_eq!(
            conclude("x_deploy", ApiCompatMode::Warn, "1.3.0", gone()),
            Ok(())
        );
        assert_eq!(
            conclude("x_deploy", ApiCompatMode::Off, "1.3.0", gone()),
            Ok(())
        );
        assert_eq!(
            conclude(
                "x_deploy",
                ApiCompatMode::Fail,
                "1.2.1",
                compared("1.2.0", &[])
            ),
            Ok(())
        );
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

    fn git_in(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn export_git_dir_writes_only_the_directory() {
        let root = std::env::temp_dir().join(format!("export-git-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let repo = root.join("repo");
        std::fs::create_dir_all(repo.join("pkg/src")).expect("dirs");
        std::fs::create_dir_all(repo.join("other")).expect("dirs");
        std::fs::write(repo.join("pkg/Cargo.toml"), "[package]\n").expect("write");
        std::fs::write(repo.join("pkg/src/lib.rs"), "pub fn a() {}\n").expect("write");
        std::fs::write(repo.join("other/x.txt"), "x").expect("write");
        std::fs::write(repo.join("other/pubspec.yaml"), "name: x\n").expect("write");
        git_in(&repo, &["init", "-q"]);
        git_in(&repo, &["add", "."]);
        git_in(&repo, &["commit", "-q", "-m", "x"]);
        git_in(&repo, &["tag", "v1.0.0"]);
        let dest = root.join("dest");
        assert_eq!(
            export_git_dir(&repo, "pkg", "v1.0.0", &dest, "Cargo.toml"),
            Ok(true)
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("src/lib.rs")).expect("lib"),
            "pub fn a() {}\n"
        );
        assert!(dest.join("Cargo.toml").exists());
        assert!(!dest.join("other").exists() && !dest.join("x.txt").exists());
        let missing = root.join("missing");
        assert_eq!(
            export_git_dir(&repo, "other", "v1.0.0", &missing, "Cargo.toml"),
            Ok(false)
        );
        assert_eq!(
            export_git_dir(&repo, "other", "v1.0.0", &root.join("pub"), "pubspec.yaml"),
            Ok(true)
        );
        assert_eq!(
            export_git_dir(&repo, "nope", "v1.0.0", &missing, "Cargo.toml"),
            Ok(false)
        );
        let status = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo)
            .output()
            .expect("status");
        assert!(status.stdout.is_empty(), "{:?}", status.stdout);
        let _ = std::fs::remove_dir_all(&root);
    }
}
