//! Re-records the cache sets and locks of nested tests: `bazel run //tools/testing:nested_cache_update -- <test label>...`.
//! `-- --from <dir>...` merges the records of several hosts instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DIR: &str = "tools/testing/nested_cache";

/// Splits `//pkg:name` into its package and name.
fn split_label(label: &str) -> Option<(&str, &str)> {
    let rest = label.strip_prefix("//")?;
    let (package, name) = rest.split_once(':')?;
    (!name.is_empty() && !name.contains('/')).then_some((package, name))
}

/// Unions `<url> <digest>` lists keyed by URL with lowercase digests, dropping `file://` entries; one URL with two digests is an error.
fn union_lists(lists: &[(String, String)]) -> Result<String, String> {
    let mut merged: BTreeMap<&str, String> = BTreeMap::new();
    for (origin, text) in lists {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let (url, sha) = line
                .split_once(' ')
                .ok_or_else(|| format!("{origin}: malformed line `{line}`"))?;
            let (url, sha) = (url.trim(), sha.trim().to_ascii_lowercase());
            if url.starts_with("file://") {
                continue;
            }
            if let Some(previous) = merged.insert(url, sha.clone())
                && previous != sha
            {
                return Err(format!("{origin}: {url} has digest {sha} and {previous}"));
            }
        }
    }
    let mut text = String::new();
    for (url, sha) in &merged {
        text.push_str(url);
        text.push(' ');
        text.push_str(sha);
        text.push('\n');
    }
    Ok(text)
}

/// Returns the one lock text shared by all hosts; differing locks are an error.
fn same_lock(locks: &[(String, String)]) -> Result<String, String> {
    let (first_origin, first) = locks.first().ok_or("no lock to merge")?;
    for (origin, text) in locks.iter().skip(1) {
        if text != first {
            return Err(format!("{origin} differs from {first_origin}"));
        }
    }
    Ok(first.clone())
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn merge(workspace: &Path, dirs: &[PathBuf]) -> Result<(), String> {
    let mut sets: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut locks: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for dir in dirs {
        let entries = std::fs::read_dir(dir)
            .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(set) = name.strip_suffix(".txt") {
                sets.entry(set.to_string()).or_default().push(dir.clone());
            } else if let Some(stem) = name.strip_suffix(".MODULE.bazel.lock") {
                let path = entry.path();
                let text = read(&path)?;
                locks
                    .entry(stem.to_string())
                    .or_default()
                    .push((path.display().to_string(), text));
            }
        }
    }
    if sets.is_empty() {
        return Err("no <set>.txt in the given directories".to_owned());
    }
    let target = workspace.join(DIR);
    let write = |name: String, text: String| -> Result<(), String> {
        let dst = target.join(&name);
        std::fs::write(&dst, text)
            .map_err(|error| format!("cannot write {}: {error}", dst.display()))?;
        println!("wrote {DIR}/{name}");
        Ok(())
    };
    for (set, set_dirs) in sets {
        let lists = set_dirs
            .iter()
            .map(|dir| {
                let path = dir.join(format!("{set}.txt"));
                Ok((path.display().to_string(), read(&path)?))
            })
            .collect::<Result<Vec<(String, String)>, String>>()?;
        write(format!("{set}.txt"), union_lists(&lists)?)?;
    }
    for (stem, texts) in locks {
        let lock = same_lock(&texts).map_err(|error| format!("{stem} lock: {error}"))?;
        write(format!("{stem}.MODULE.bazel.lock"), lock)?;
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .ok_or("run with `bazel run //tools/testing:nested_cache_update -- <test label>...`")?;
    let labels: Vec<String> = std::env::args().skip(1).collect();
    if let Some((first, rest)) = labels.split_first()
        && first == "--from"
    {
        let dirs: Vec<PathBuf> = rest.iter().map(PathBuf::from).collect();
        if dirs.is_empty() {
            return Err("pass the directories after --from".to_owned());
        }
        return merge(&workspace, &dirs);
    }
    if labels.is_empty() {
        return Err("pass the labels of the nested tests to record".to_owned());
    }
    let parts = labels
        .iter()
        .map(|label| {
            split_label(label).ok_or_else(|| format!("{label} is not a //package:name label"))
        })
        .collect::<Result<Vec<(&str, &str)>, String>>()?;
    let test_command = format!(
        "bazel test {} --config=nested_record --nocache_test_results --nozip_undeclared_test_outputs --test_tag_filters= --build_tag_filters=",
        labels.join(" ")
    );
    let target = workspace.join(DIR);
    for (package, name) in parts {
        let recorded = workspace
            .join("bazel-testlogs")
            .join(package)
            .join(name)
            .join("test.outputs")
            .join("nested_cache");
        let entries = std::fs::read_dir(&recorded).map_err(|error| {
            format!(
                "{} recorded nothing: {error}; run `{test_command}` first",
                recorded.display()
            )
        })?;
        for entry in entries.filter_map(Result::ok) {
            let dst = target.join(entry.file_name());
            std::fs::copy(entry.path(), &dst)
                .map_err(|error| format!("cannot write {}: {error}", dst.display()))?;
            println!("wrote {DIR}/{}", entry.file_name().to_string_lossy());
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("nested_cache_update: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(text: &str) -> (String, String) {
        ("host".to_string(), text.to_string())
    }

    #[test]
    fn union_keys_by_url_and_sorts() {
        let merged = union_lists(&[
            list("https://b x\nhttps://a y\n"),
            list("https://c z\nhttps://a y\n"),
        ])
        .unwrap();
        assert_eq!(merged, "https://a y\nhttps://b x\nhttps://c z\n");
    }

    #[test]
    fn union_drops_file_urls() {
        let merged = union_lists(&[list("file:///tmp/x/tool a\nhttps://a y\n")]).unwrap();
        assert_eq!(merged, "https://a y\n");
    }

    #[test]
    fn union_lowercases_sha256() {
        let merged = union_lists(&[list("https://a 0F0B\n")]).unwrap();
        assert_eq!(merged, "https://a 0f0b\n");
    }

    #[test]
    fn union_fails_on_sha256_conflict() {
        let error = union_lists(&[list("https://a one\n"), list("https://a two\n")]).unwrap_err();
        assert!(error.contains("https://a"), "{error}");
    }

    #[test]
    fn lock_fails_when_locks_differ() {
        assert_eq!(same_lock(&[list("{}"), list("{}")]).unwrap(), "{}");
        assert!(same_lock(&[list("{}"), list("{\"a\":1}")]).is_err());
    }
}
