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

/// Returns the highest non-yanked, non-prerelease version in a sparse index body.
pub fn latest_published(index_body: &str) -> Option<String> {
    let mut best: Option<([u64; 3], String)> = None;
    for line in index_body.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let version = entry["vers"].as_str().unwrap_or("");
        if entry["yanked"].as_bool().unwrap_or(false) || version.contains('-') {
            continue;
        }
        let Some(parsed) = parse_version(version) else {
            continue;
        };
        if best.as_ref().is_none_or(|(top, _)| parsed > *top) {
            best = Some((parsed, version.to_owned()));
        }
    }
    best.map(|(_, version)| version)
}

/// Returns the highest `v<version>` tag below `next` from `git tag` output.
pub fn previous_tag(tags: &str, next: &str) -> Option<String> {
    let next = parse_version(next)?;
    tags.lines()
        .filter_map(|tag| {
            let parsed = parse_version(tag.trim().strip_prefix('v')?)?;
            (parsed < next).then(|| (parsed, tag.trim().to_owned()))
        })
        .max()
        .map(|(_, tag)| tag)
}

/// Returns the highest release below `next` listed in a `maven-metadata.xml` body.
pub fn previous_release(metadata: &str, next: &str) -> Option<String> {
    let next = parse_version(next)?;
    metadata
        .split("<version>")
        .skip(1)
        .filter_map(|part| {
            let version = part.split("</version>").next()?.trim();
            let parsed = parse_version(version)?;
            (!version.contains('-') && parsed < next).then(|| (parsed, version.to_owned()))
        })
        .max()
        .map(|(_, version)| version)
}

/// Writes the files under `rel_dir` at `rev` into `dest`; false when that directory has no `Cargo.toml`.
/// Symlinks and submodules are skipped. The index and working tree are not touched.
pub fn export_git_dir(
    workspace: &std::path::Path,
    rel_dir: &str,
    rev: &str,
    dest: &std::path::Path,
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
        found |= rel == "Cargo.toml";
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
        assert_eq!(latest_published(body).as_deref(), Some("1.10.0"));
        assert_eq!(latest_published(""), None);
    }

    #[test]
    fn previous_tag_picks_highest_below_next() {
        let tags = "v1.2.0\nv1.10.0\nv2.0.0\nnightly\n";
        assert_eq!(previous_tag(tags, "2.0.0").as_deref(), Some("v1.10.0"));
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
        git_in(&repo, &["init", "-q"]);
        git_in(&repo, &["add", "."]);
        git_in(&repo, &["commit", "-q", "-m", "x"]);
        git_in(&repo, &["tag", "v1.0.0"]);
        let dest = root.join("dest");
        assert_eq!(export_git_dir(&repo, "pkg", "v1.0.0", &dest), Ok(true));
        assert_eq!(
            std::fs::read_to_string(dest.join("src/lib.rs")).expect("lib"),
            "pub fn a() {}\n"
        );
        assert!(dest.join("Cargo.toml").exists());
        assert!(!dest.join("other").exists() && !dest.join("x.txt").exists());
        let missing = root.join("missing");
        assert_eq!(
            export_git_dir(&repo, "other", "v1.0.0", &missing),
            Ok(false)
        );
        assert_eq!(export_git_dir(&repo, "nope", "v1.0.0", &missing), Ok(false));
        let status = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo)
            .output()
            .expect("status");
        assert!(status.stdout.is_empty(), "{:?}", status.stdout);
        let _ = std::fs::remove_dir_all(&root);
    }
}
