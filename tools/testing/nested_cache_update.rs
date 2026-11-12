//! Re-records the cache sets and locks of nested tests: `bazel run //tools/testing:nested_cache_update -- <test label>...`.
//! `-- --from <dir>...` merges the records of several hosts instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DIR: &str = "tools/testing/nested_cache";

fn fail(message: &str) -> ! {
    eprintln!("nested_cache_update: {message}");
    std::process::exit(1);
}

/// Splits `//pkg:name` into its package and name.
fn split_label(label: &str) -> Option<(&str, &str)> {
    let rest = label.strip_prefix("//")?;
    let (package, name) = rest.split_once(':')?;
    (!name.is_empty() && !name.contains('/')).then_some((package, name))
}

/// Unions `<url> <sha256>` lists keyed by URL; one URL with two sha256 values is an error.
fn union_lists(lists: &[(String, String)]) -> Result<String, String> {
    let mut merged: BTreeMap<&str, &str> = BTreeMap::new();
    for (origin, text) in lists {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let (url, sha) = line
                .split_once(' ')
                .ok_or_else(|| format!("{origin}: malformed line `{line}`"))?;
            let (url, sha) = (url.trim(), sha.trim());
            if let Some(previous) = merged.insert(url, sha) {
                if previous != sha {
                    return Err(format!("{origin}: {url} has sha256 {sha} and {previous}"));
                }
            }
        }
    }
    Ok(merged.iter().map(|(url, sha)| format!("{url} {sha}\n")).collect())
}

/// Returns the one lock text shared by all hosts; differing locks are an error.
fn same_lock(locks: &[(String, String)]) -> Result<String, String> {
    let (first_origin, first) = locks.first().ok_or("no lock to merge")?;
    for (origin, text) in &locks[1..] {
        if text != first {
            return Err(format!("{origin} differs from {first_origin}"));
        }
    }
    Ok(first.clone())
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", path.display())))
}

fn merge(workspace: &Path, dirs: &[PathBuf]) {
    let mut sets: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for dir in dirs {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", dir.display())));
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(set) = name.strip_suffix(".txt") {
                sets.entry(set.to_string()).or_default().push(dir.clone());
            }
        }
    }
    if sets.is_empty() {
        fail("no <set>.txt in the given directories");
    }
    let target = workspace.join(DIR);
    for (set, set_dirs) in sets {
        let collect = |suffix: &str| -> Vec<(String, String)> {
            set_dirs
                .iter()
                .filter(|dir| dir.join(format!("{set}{suffix}")).exists())
                .map(|dir| {
                    let path = dir.join(format!("{set}{suffix}"));
                    (path.display().to_string(), read(&path))
                })
                .collect()
        };
        let list = union_lists(&collect(".txt")).unwrap_or_else(|error| fail(&error));
        let lock = same_lock(&collect(".MODULE.bazel.lock")).unwrap_or_else(|error| fail(&format!("{set} lock: {error}")));
        for (name, text) in [(format!("{set}.txt"), list), (format!("{set}.MODULE.bazel.lock"), lock)] {
            let dst = target.join(&name);
            std::fs::write(&dst, text).unwrap_or_else(|error| fail(&format!("cannot write {}: {error}", dst.display())));
            println!("wrote {DIR}/{name}");
        }
    }
}

fn main() {
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("run with `bazel run //tools/testing:nested_cache_update -- <test label>...`"));
    let labels: Vec<String> = std::env::args().skip(1).collect();
    if labels.first().map(String::as_str) == Some("--from") {
        let dirs: Vec<PathBuf> = labels[1..].iter().map(PathBuf::from).collect();
        if dirs.is_empty() {
            fail("pass the directories after --from");
        }
        merge(&workspace, &dirs);
        return;
    }
    if labels.is_empty() {
        fail("pass the labels of the nested tests to record");
    }
    let parts: Vec<(&str, &str)> = labels
        .iter()
        .map(|label| split_label(label).unwrap_or_else(|| fail(&format!("{label} is not a //package:name label"))))
        .collect();
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
        let entries = std::fs::read_dir(&recorded)
            .unwrap_or_else(|error| fail(&format!("{} recorded nothing: {error}; run `{test_command}` first", recorded.display())));
        for entry in entries.filter_map(Result::ok) {
            let dst = target.join(entry.file_name());
            std::fs::copy(entry.path(), &dst)
                .unwrap_or_else(|error| fail(&format!("cannot write {}: {error}", dst.display())));
            println!("wrote {DIR}/{}", entry.file_name().to_string_lossy());
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
        let merged = union_lists(&[list("https://b x\nhttps://a y\n"), list("https://c z\nhttps://a y\n")]).unwrap();
        assert_eq!(merged, "https://a y\nhttps://b x\nhttps://c z\n");
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
