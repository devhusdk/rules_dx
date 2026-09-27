#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn read_lines(path: &Path) -> std::io::Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.split('\n').map(str::to_owned).collect())
}

fn fnmatch(pattern: &str, name: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = name.chars().collect();
    let mut px = 0usize;
    let mut sx = 0usize;
    let mut star: Option<usize> = None;
    let mut match_idx = 0usize;
    while sx < s.len() {
        if px < pat.len() && (pat[px] == '?' || pat[px] == s[sx]) {
            px += 1;
            sx += 1;
        } else if px < pat.len() && pat[px] == '*' {
            star = Some(px);
            match_idx = sx;
            px += 1;
        } else if let Some(star_idx) = star {
            px = star_idx + 1;
            match_idx += 1;
            sx = match_idx;
        } else {
            return false;
        }
    }
    while px < pat.len() && pat[px] == '*' {
        px += 1;
    }
    px == pat.len()
}

fn skip_dir(name: &str, extra: &BTreeSet<String>) -> bool {
    if fnmatch("bazel-*", name) || name == ".git" {
        return true;
    }
    extra.contains(name)
}

fn include_ok(basename: &str, includes: &[String]) -> bool {
    if includes.is_empty() {
        return true;
    }
    includes.iter().any(|pat| fnmatch(pat, basename))
}

pub fn resolve_runfiles(rel: &str) -> PathBuf {
    let direct = PathBuf::from(rel);
    if direct.is_absolute() || direct.exists() {
        return direct;
    }
    for key in ["TEST_SRCDIR", "RUNFILES_DIR"] {
        if let Ok(root) = std::env::var(key) {
            let root = PathBuf::from(root);
            for candidate in [
                root.join("_main").join(rel),
                root.join("rules_dx").join(rel),
                root.join(rel),
            ] {
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }
    direct
}

pub fn file_contains(path: &Path, patterns: &[&str], fixed: bool) -> std::io::Result<bool> {
    let lines = read_lines(path)?;
    if fixed {
        for pat in patterns {
            if !lines.iter().any(|line| line.contains(pat)) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    let compiled = patterns
        .iter()
        .map(|pat| regex::Regex::new(pat))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err.to_string()))?;
    for rx in &compiled {
        if !lines.iter().any(|line| rx.is_match(line)) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn file_absent(path: &Path, patterns: &[&str], fixed: bool) -> std::io::Result<bool> {
    Ok(!file_contains(path, patterns, fixed)?)
}

pub fn expect_contains(path: &Path, patterns: &[&str]) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing file {}", path.display()));
    }
    let lines = read_lines(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut missing = Vec::new();
    for pat in patterns {
        if !lines.iter().any(|line| line.contains(pat)) {
            missing.push(format!("[{pat}]"));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} missing literals:{}",
            path.display(),
            missing.join(" ")
        ))
    }
}

pub fn expect_absent(path: &Path, patterns: &[&str]) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing file {}", path.display()));
    }
    let lines = read_lines(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut present = Vec::new();
    for pat in patterns {
        if lines.iter().any(|line| line.contains(pat)) {
            present.push(format!("[{pat}]"));
        }
    }
    if present.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} must not contain:{}",
            path.display(),
            present.join(" ")
        ))
    }
}

pub fn expect_re_contains(path: &Path, patterns: &[&str]) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing file {}", path.display()));
    }
    let lines = read_lines(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut missing = Vec::new();
    for pat in patterns {
        let rx = regex::Regex::new(pat).map_err(|err| format!("bad pattern [{pat}]: {err}"))?;
        if !lines.iter().any(|line| rx.is_match(line)) {
            missing.push(format!("[{pat}]"));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} missing patterns:{}",
            path.display(),
            missing.join(" ")
        ))
    }
}

pub fn expect_re_absent(path: &Path, patterns: &[&str]) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing file {}", path.display()));
    }
    let lines = read_lines(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut present = Vec::new();
    for pat in patterns {
        let rx = regex::Regex::new(pat).map_err(|err| format!("bad pattern [{pat}]: {err}"))?;
        if lines.iter().any(|line| rx.is_match(line)) {
            present.push(format!("[{pat}]"));
        }
    }
    if present.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} must not match:{}",
            path.display(),
            present.join(" ")
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct TreeOptions {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub allows: Vec<String>,
    pub allow_paths: Vec<String>,
}

fn line_matches(line: &str, patterns: &[&str], fixed: bool) -> bool {
    if fixed {
        return patterns.iter().any(|pat| line.contains(pat));
    }
    patterns.iter().any(|pat| {
        regex::Regex::new(pat)
            .map(|rx| rx.is_match(line))
            .unwrap_or(false)
    })
}

pub fn hermetic_walk(roots: &[PathBuf], options: &TreeOptions) -> Vec<PathBuf> {
    let exclude_set: BTreeSet<String> = options.excludes.iter().cloned().collect();
    let extra_dirs: BTreeSet<String> = options.exclude_dirs.iter().cloned().collect();
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = roots.to_vec();
    while let Some(root) = stack.pop() {
        if root.is_file() {
            if let Some(name) = root.file_name().and_then(|n| n.to_str()) {
                if exclude_set.contains(name) {
                    continue;
                }
                if include_ok(name, &options.includes) {
                    out.push(root);
                }
            }
            continue;
        }
        if !root.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !skip_dir(&name, &extra_dirs) {
                    dirs.push(path);
                }
            } else {
                if exclude_set.contains(&name) {
                    continue;
                }
                if include_ok(&name, &options.includes) {
                    files.push(path);
                }
            }
        }
        dirs.sort();
        files.sort();
        out.extend(files);
        dirs.reverse();
        stack.extend(dirs);
    }
    out.sort();
    out
}

fn tree_hits(
    roots: &[PathBuf],
    patterns: &[&str],
    fixed: bool,
    options: &TreeOptions,
) -> Vec<String> {
    let mut hits = Vec::new();
    for path in hermetic_walk(roots, options) {
        let rel = path.to_string_lossy().into_owned();
        if options.allow_paths.iter().any(|sub| rel.contains(sub)) {
            continue;
        }
        let lines = match read_lines(&path) {
            Ok(lines) => lines,
            Err(_) => continue,
        };
        for text in lines {
            if !line_matches(&text, patterns, fixed) {
                continue;
            }
            if options.allows.iter().any(|sub| text.contains(sub)) {
                continue;
            }
            let tagged = format!("{rel}:{text}");
            if options.allows.iter().any(|sub| tagged.contains(sub)) {
                continue;
            }
            hits.push(tagged);
            return hits;
        }
    }
    hits
}

pub fn tree_contains(
    roots: &[PathBuf],
    patterns: &[&str],
    fixed: bool,
    options: &TreeOptions,
) -> bool {
    !tree_hits(roots, patterns, fixed, options).is_empty()
}

pub fn tree_absent(
    roots: &[PathBuf],
    patterns: &[&str],
    fixed: bool,
    options: &TreeOptions,
) -> bool {
    tree_hits(roots, patterns, fixed, options).is_empty()
}

pub fn tree_list(
    roots: &[PathBuf],
    patterns: &[&str],
    fixed: bool,
    options: &TreeOptions,
) -> Vec<PathBuf> {
    let mut seen = Vec::new();
    let mut seen_set = BTreeSet::new();
    for path in hermetic_walk(roots, options) {
        let lines = match read_lines(&path) {
            Ok(lines) => lines,
            Err(_) => continue,
        };
        if lines.iter().any(|line| line_matches(line, patterns, fixed)) {
            let key = path.to_string_lossy().into_owned();
            if seen_set.insert(key) {
                seen.push(path);
            }
        }
    }
    seen
}

pub fn tree_count(
    roots: &[PathBuf],
    patterns: &[&str],
    fixed: bool,
    options: &TreeOptions,
) -> usize {
    let mut total = 0usize;
    for path in hermetic_walk(roots, options) {
        let lines = match read_lines(&path) {
            Ok(lines) => lines,
            Err(_) => continue,
        };
        for line in lines {
            if line_matches(&line, patterns, fixed) {
                total += 1;
            }
        }
    }
    total
}

fn anchor_matches(line: &str, anchor: &str, anchor_fixed: bool) -> bool {
    if anchor_fixed {
        return line.contains(anchor);
    }
    regex::Regex::new(anchor)
        .map(|rx| rx.is_match(line))
        .unwrap_or(false)
}

pub fn context_contains(
    path: &Path,
    anchor: &str,
    after: usize,
    anchor_fixed: bool,
    patterns: &[&str],
    fixed: bool,
) -> std::io::Result<bool> {
    let lines = read_lines(path)?;
    let window = after + 1;
    for (idx, line) in lines.iter().enumerate() {
        if !anchor_matches(line, anchor, anchor_fixed) {
            continue;
        }
        let end = (idx + window).min(lines.len());
        for text in &lines[idx..end] {
            if line_matches(text, patterns, fixed) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn context_absent(
    path: &Path,
    anchor: &str,
    after: usize,
    anchor_fixed: bool,
    patterns: &[&str],
    fixed: bool,
) -> std::io::Result<bool> {
    Ok(!context_contains(
        path,
        anchor,
        after,
        anchor_fixed,
        patterns,
        fixed,
    )?)
}

pub fn extract_quoted(path: &Path, lit: &str) -> std::io::Result<Option<String>> {
    let lines = read_lines(path)?;
    for line in lines {
        if !line.contains(lit) {
            continue;
        }
        if let Some(first) = line.find('"') {
            if let Some(last) = line.rfind('"') {
                if last > first {
                    return Ok(Some(line[first + 1..last].to_owned()));
                }
            }
        }
    }
    Ok(None)
}

pub fn extract_re(path: &Path, pattern: &str) -> std::io::Result<Option<String>> {
    let lines = read_lines(path)?;
    let rx = regex::Regex::new(pattern)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err.to_string()))?;
    for line in lines {
        if let Some(found) = rx.find(&line) {
            return Ok(Some(found.as_str().to_owned()));
        }
    }
    Ok(None)
}

fn workspace_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BUILD_WORKSPACE_DIRECTORY") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if text.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(text))
                }
            } else {
                None
            }
        })
}

fn update_expect() -> bool {
    std::env::var("UPDATE_EXPECT").as_deref() == Ok("1")
}

fn stage_update(actual: &Path, label: &str) -> Option<PathBuf> {
    let out_dir = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("TMPDIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    std::fs::create_dir_all(&out_dir).ok()?;
    let staged = out_dir.join(format!("{label}.expected.update"));
    std::fs::copy(actual, &staged).ok()?;
    Some(staged)
}

fn unified_diff(expected: &str, actual: &str) -> String {
    let old: Vec<&str> = expected.lines().collect();
    let new: Vec<&str> = actual.lines().collect();
    let mut out = vec!["--- expected".to_owned(), "+++ actual".to_owned()];
    let max = old.len().max(new.len());
    for i in 0..max {
        let old_line = old.get(i).copied().unwrap_or("");
        let new_line = new.get(i).copied().unwrap_or("");
        if old_line != new_line {
            if i < old.len() {
                out.push(format!("-{old_line}"));
            }
            if i < new.len() {
                out.push(format!("+{new_line}"));
            }
        }
    }
    out.join("\n")
}

pub fn snapshot_diff(
    expected: &Path,
    actual: &Path,
    workspace_rel: Option<&str>,
) -> Result<(), String> {
    let expected_bytes = std::fs::read(expected)
        .map_err(|err| format!("cannot read {}: {err}", expected.display()))?;
    let actual_bytes =
        std::fs::read(actual).map_err(|err| format!("cannot read {}: {err}", actual.display()))?;
    if expected_bytes == actual_bytes {
        return Ok(());
    }
    if update_expect() {
        if let Some(rel) = workspace_rel {
            if let Some(root) = workspace_root() {
                if root.is_dir() {
                    let dest = root.join(rel);
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::copy(actual, &dest)
                        .map_err(|err| format!("cannot refresh {}: {err}", dest.display()))?;
                    return Ok(());
                }
            }
        }
        let expected_text = expected.to_string_lossy();
        if !expected_text.contains("/runfiles/") && std::fs::copy(actual, expected).is_ok() {
            return Ok(());
        }
        let label = expected
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("expected");
        if let Some(staged) = stage_update(actual, label) {
            let mut hint = format!("staged refreshed golden at {}", staged.display());
            if let Some(rel) = workspace_rel {
                hint.push_str(&format!("; copy it to {rel} in the checkout, then re-run"));
            }
            return Ok(());
        }
        return Err("UPDATE_EXPECT staged refresh failed".to_owned());
    }
    let expected_text = String::from_utf8_lossy(&expected_bytes);
    let actual_text = String::from_utf8_lossy(&actual_bytes);
    let diff = unified_diff(&expected_text, &actual_text);
    Err(format!(
        "snapshot FAIL: {} differs from actual\n{diff}\nre-run with UPDATE_EXPECT=1 to refresh the golden, then review the diff before committing",
        expected.display()
    ))
}

pub fn snapshot_canonical_json_diff(
    expected: &Path,
    actual: &Path,
    workspace_rel: Option<&str>,
) -> Result<(), String> {
    let expected_text = std::fs::read_to_string(expected)
        .map_err(|err| format!("cannot read {}: {err}", expected.display()))?;
    let actual_text = std::fs::read_to_string(actual)
        .map_err(|err| format!("cannot read {}: {err}", actual.display()))?;
    let expected_value: serde_json::Value = serde_json::from_str(&expected_text)
        .map_err(|err| format!("non-JSON input {}: {err}", expected.display()))?;
    let actual_value: serde_json::Value = serde_json::from_str(&actual_text)
        .map_err(|err| format!("non-JSON input {}: {err}", actual.display()))?;
    let mut expected_canonical =
        serde_json::to_string_pretty(&expected_value).map_err(|err| err.to_string())?;
    expected_canonical.push('\n');
    let mut actual_canonical =
        serde_json::to_string_pretty(&actual_value).map_err(|err| err.to_string())?;
    actual_canonical.push('\n');
    if expected_canonical == actual_canonical {
        return Ok(());
    }
    if update_expect() {
        if let Some(rel) = workspace_rel {
            if let Some(root) = workspace_root() {
                if root.is_dir() {
                    let dest = root.join(rel);
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::write(&dest, actual_canonical.as_bytes())
                        .map_err(|err| format!("cannot refresh {}: {err}", dest.display()))?;
                    return Ok(());
                }
            }
        }
        let expected_text = expected.to_string_lossy();
        if !expected_text.contains("/runfiles/")
            && std::fs::write(expected, actual_canonical.as_bytes()).is_ok()
        {
            return Ok(());
        }
        let scratch = std::env::temp_dir().join("dx-snapshot-canonical");
        let _ = std::fs::create_dir_all(&scratch);
        let staged = scratch.join("actual.canonical.json");
        std::fs::write(&staged, actual_canonical.as_bytes())
            .map_err(|err| format!("cannot stage canonical JSON: {err}"))?;
        return Ok(());
    }
    let diff = unified_diff(&expected_canonical, &actual_canonical);
    Err(format!(
        "snapshot FAIL (canonical JSON): {} differs\n{diff}\nre-run with UPDATE_EXPECT=1 to refresh the golden, then review before committing",
        expected.display()
    ))
}

pub fn assert_valid_json(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map(|_| ())
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(prefix: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("scratch tempdir")
    }

    fn write(dir: &Path, rel: &str, text: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parents");
        }
        let mut handle = std::fs::File::create(&path).expect("create");
        handle.write_all(text.as_bytes()).expect("write");
        path
    }

    #[test]
    fn contains_fixed() {
        let dir = scratch("dx-testing-");
        let path = write(
            dir.path(),
            "a.txt",
            "extra_target_triples = True\nnothing here\n",
        );
        assert!(file_contains(&path, &["extra_target_triples"], true).expect("read"));
        assert!(!file_contains(&path, &["missing-lit"], true).expect("read"));
    }

    #[test]
    fn contains_all_patterns() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "b.txt", "alpha\nbeta\ngamma\n");
        assert!(file_contains(&path, &["alpha", "beta"], true).expect("read"));
        assert!(!file_contains(&path, &["alpha", "nope"], true).expect("read"));
    }

    #[test]
    fn contains_re() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "c.txt", "version = \"1.26.6\"\n");
        assert!(file_contains(&path, &[r#"version = "[^"]+""#], false).expect("read"));
        assert!(!file_contains(&path, &[r"no-match-\d+"], false).expect("read"));
    }

    #[test]
    fn absent_modes() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "d.txt", "clean file\n");
        assert!(file_absent(&path, &["forbidden"], true).expect("read"));
        assert!(!file_absent(&path, &["clean"], true).expect("read"));
    }

    #[test]
    fn missing_file_fails_closed() {
        let dir = scratch("dx-testing-");
        let missing = dir.path().join("nope.txt");
        assert!(file_contains(&missing, &["x"], true).is_err());
        assert!(file_absent(&missing, &["x"], true).is_err());
    }

    #[test]
    fn tree_contains_include() {
        let dir = scratch("dx-testing-");
        write(dir.path(), "keep.yml", "runs-on: windows-latest\n");
        write(dir.path(), "skip.md", "runs-on: windows-latest\n");
        let roots = vec![dir.path().to_path_buf()];
        let options = TreeOptions {
            includes: vec!["*.yml".to_owned()],
            ..Default::default()
        };
        assert!(tree_contains(
            &roots,
            &["runs-on: windows-latest"],
            true,
            &options
        ));
        let options = TreeOptions {
            includes: vec!["*.txt".to_owned()],
            ..Default::default()
        };
        assert!(!tree_contains(
            &roots,
            &["runs-on: windows-latest"],
            true,
            &options
        ));
    }

    #[test]
    fn tree_absent_with_exclude_and_allow() {
        let dir = scratch("dx-testing-");
        write(dir.path(), "self.sh", "Installed Build Tools\n");
        write(
            dir.path(),
            "other.sh",
            "Installed Build Tools never approved\n",
        );
        let roots = vec![dir.path().to_path_buf()];
        let options = TreeOptions {
            excludes: vec!["self.sh".to_owned()],
            allows: vec!["never approved".to_owned()],
            ..Default::default()
        };
        assert!(tree_absent(
            &roots,
            &["Installed Build Tools"],
            true,
            &options
        ));
        let options = TreeOptions {
            excludes: vec!["self.sh".to_owned()],
            ..Default::default()
        };
        assert!(!tree_absent(
            &roots,
            &["Installed Build Tools"],
            true,
            &options
        ));
    }

    #[test]
    fn tree_skips_vcs_and_bazel_dirs() {
        let dir = scratch("dx-testing-");
        write(dir.path(), ".git/hidden.txt", "needle\n");
        write(dir.path(), "bazel-out/gen.txt", "needle\n");
        write(dir.path(), "visible.txt", "hay\n");
        let roots = vec![dir.path().to_path_buf()];
        assert!(!tree_contains(
            &roots,
            &["needle"],
            true,
            &TreeOptions::default()
        ));
    }

    #[test]
    fn tree_list_filters_by_include() {
        let dir = scratch("dx-testing-");
        write(dir.path(), "gazelle/a/BUILD.bazel", "gazelle_binary\n");
        write(dir.path(), "gazelle/b.txt", "gazelle_binary\n");
        let roots = vec![dir.path().to_path_buf()];
        let options = TreeOptions {
            includes: vec!["BUILD.bazel".to_owned()],
            ..Default::default()
        };
        let listed = tree_list(&roots, &["gazelle_binary"], true, &options);
        assert_eq!(listed.len(), 1);
        assert!(listed[0].to_string_lossy().contains("BUILD.bazel"));
    }

    #[test]
    fn context_contains_fixed() {
        let dir = scratch("dx-testing-");
        let path = write(
            dir.path(),
            "ci.yml",
            "build-windows-x86_64\nline1\nline2 secrets.leak\n",
        );
        assert!(
            context_contains(&path, "build-windows-x86_64", 30, true, &["secrets."], true)
                .expect("read")
        );
        assert!(
            !context_contains(&path, "build-windows-x86_64", 1, true, &["secrets."], true)
                .expect("read")
        );
    }

    #[test]
    fn context_anchor_re() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "ci2.yml", "  test:\n    needs: [build]\n");
        assert!(
            context_contains(&path, r"^  test:", 3, false, &["needs: [build]"], true)
                .expect("read")
        );
    }

    #[test]
    fn context_absent_re() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "ci3.yml", "build-macos-arm64\nline1\nclean\n");
        assert!(context_absent(
            &path,
            "build-macos-arm64",
            20,
            true,
            &[r"secrets\.|GH_TOKEN"],
            false
        )
        .expect("read"));
    }

    #[test]
    fn extract_quoted_value() {
        let dir = scratch("dx-testing-");
        let path = write(
            dir.path(),
            "codegen.bzl",
            "DX_CODEGEN_PLAN_OUTPUT_GROUP = \"dx_codegen_plans\"\n",
        );
        assert_eq!(
            extract_quoted(&path, "DX_CODEGEN_PLAN_OUTPUT_GROUP = ").expect("read"),
            Some("dx_codegen_plans".to_owned())
        );
    }

    #[test]
    fn extract_re_match() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "pins.bzl", "GO_SDK_VERSION = \"1.26.6\"\n");
        assert_eq!(
            extract_re(&path, r#""[^"]+"$"#).expect("read"),
            Some("\"1.26.6\"".to_owned())
        );
    }

    #[test]
    fn extract_missing_returns_none() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "empty.txt", "nothing\n");
        assert_eq!(extract_quoted(&path, "MISSING").expect("read"), None);
        assert_eq!(extract_re(&path, r"\d+").expect("read"), None);
    }

    #[test]
    fn tree_count_matches_grep_wc() {
        let dir = scratch("dx-testing-");
        write(dir.path(), "one.yml", "pin\npin\n");
        write(dir.path(), "two.yml", "pin\n");
        write(dir.path(), "skip.md", "pin\n");
        let roots = vec![dir.path().to_path_buf()];
        let options = TreeOptions {
            includes: vec!["*.yml".to_owned()],
            ..Default::default()
        };
        assert_eq!(tree_count(&roots, &["pin"], true, &options), 3);
    }

    #[test]
    fn expect_helpers_report_missing_and_present() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "f.txt", "alpha\nbeta\n");
        assert!(expect_contains(&path, &["alpha", "beta"]).is_ok());
        assert!(expect_contains(&path, &["alpha", "nope"]).is_err());
        assert!(expect_absent(&path, &["forbidden"]).is_ok());
        assert!(expect_absent(&path, &["alpha"]).is_err());
        assert!(expect_re_contains(&path, &[r"alp.*"]).is_ok());
        assert!(expect_re_absent(&path, &[r"alp.*"]).is_err());
    }

    #[test]
    fn snapshot_pass_and_fail() {
        let dir = scratch("dx-testing-");
        let expected = write(dir.path(), "expected.txt", "same\n");
        let actual = write(dir.path(), "actual.txt", "same\n");
        assert!(snapshot_diff(&expected, &actual, None).is_ok());
        let drifted = write(dir.path(), "drifted.txt", "other\n");
        assert!(snapshot_diff(&expected, &drifted, None).is_err());
    }

    #[test]
    fn snapshot_canonical_json_ignores_key_order() {
        let dir = scratch("dx-testing-");
        let expected = write(dir.path(), "e.json", "{\"b\": 1, \"a\": 2}\n");
        let actual = write(dir.path(), "a.json", "{\"a\": 2, \"b\": 1}\n");
        assert!(snapshot_canonical_json_diff(&expected, &actual, None).is_ok());
        let drifted = write(dir.path(), "d.json", "{\"a\": 3, \"b\": 1}\n");
        assert!(snapshot_canonical_json_diff(&expected, &drifted, None).is_err());
        assert!(assert_valid_json(&expected).is_ok());
        let invalid = write(dir.path(), "bad.json", "not json\n");
        assert!(assert_valid_json(&invalid).is_err());
    }
}
