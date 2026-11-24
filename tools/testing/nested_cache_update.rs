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

/// Unions `<url> <digest>` lists keyed by URL with lowercase digests, dropping `file://` entries and
/// `id-` entries whose digest is left without an https URL; one URL with two digests is an error.
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
    let fetchable: std::collections::BTreeSet<String> = merged
        .iter()
        .filter(|(url, _)| url.starts_with("https://"))
        .map(|(_, sha)| sha.clone())
        .collect();
    merged.retain(|url, sha| !url.starts_with("id-") || fetchable.contains(sha));
    let mut text = String::new();
    for (url, sha) in &merged {
        text.push_str(url);
        text.push(' ');
        text.push_str(sha);
        text.push('\n');
    }
    Ok(text)
}

type Json = serde_json::Map<String, serde_json::Value>;

/// Rewrites raw and JSON-escaped CRLF line endings to LF.
fn lf(text: &str) -> String {
    // Windows locks carry CRLF inside generated file contents, both raw and as `\r\n` escapes.
    text.replace("\r\n", "\n").replace("\\r\\n", "\\n")
}

/// Parses a lock as a JSON object.
fn parse_lock(origin: &str, text: &str) -> Result<Json, String> {
    match serde_json::from_str(text) {
        Ok(serde_json::Value::Object(lock)) => Ok(lock),
        Ok(_) => Err(format!("{origin} is not a JSON object")),
        Err(error) => Err(format!("{origin}: {error}")),
    }
}

/// Returns the `moduleExtensions` object of a lock, empty when it has none.
fn extensions(lock: &Json) -> Json {
    match lock.get("moduleExtensions") {
        Some(serde_json::Value::Object(map)) => map.clone(),
        _ => Json::new(),
    }
}

/// The body line range of one extension in a lock and its `(key, lines)` entries.
type Entries<'a> = (std::ops::Range<usize>, Vec<(String, Vec<&'a str>)>);

/// Finds the entries of `extension` in a Bazel-written lock.
fn extension_entries<'a>(lines: &[&'a str], extension: &str) -> Result<Entries<'a>, String> {
    let name = serde_json::to_string(extension).map_err(|error| error.to_string())?;
    let header = format!("    {name}: {{");
    let start = lines
        .iter()
        .position(|line| *line == header)
        .ok_or_else(|| format!("no `{header}` line"))?
        + 1;
    let mut entries: Vec<(String, Vec<&str>)> = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start) {
        if matches!(*line, "    }" | "    },") {
            return Ok((start..index, entries));
        }
        if let Some(key) = line
            .strip_prefix("      \"")
            .and_then(|rest| rest.strip_suffix("\": {"))
        {
            entries.push((key.to_owned(), vec![*line]));
        } else if let Some((_, block)) = entries.last_mut() {
            block.push(if *line == "      }," { "      }" } else { line });
        } else {
            return Err(format!("{extension}: unexpected line `{line}`"));
        }
    }
    Err(format!("{extension} is not closed"))
}

/// Merges the hosts' locks. Every field must match, except that an extension Bazel evaluates per
/// host (keys like `os:linux,arch:amd64`) keeps each host's entry, sorted by key as Bazel writes them.
fn merge_locks(locks: &[(String, String)]) -> Result<String, String> {
    let locks: Vec<(String, String)> = locks
        .iter()
        .map(|(origin, text)| (origin.clone(), lf(text)))
        .collect();
    let (first_origin, first) = locks.first().ok_or("no lock to merge")?;
    let base = parse_lock(first_origin, first)?;
    let mut merged = extensions(&base);
    let mut per_host: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (origin, text) in locks.iter().skip(1) {
        let lock = parse_lock(origin, text)?;
        let mut rest = lock.clone();
        let mut base_rest = base.clone();
        rest.remove("moduleExtensions");
        base_rest.remove("moduleExtensions");
        if rest != base_rest {
            return Err(format!(
                "{origin} differs from {first_origin} outside moduleExtensions"
            ));
        }
        let theirs = extensions(&lock);
        if theirs.keys().ne(merged.keys()) {
            return Err(format!("{origin} has other extensions than {first_origin}"));
        }
        for (extension, entries) in theirs {
            let (Some(serde_json::Value::Object(ours)), serde_json::Value::Object(entries)) =
                (merged.get_mut(&extension), entries)
            else {
                return Err(format!("{origin}: {extension} is not an object"));
            };
            for (key, entry) in entries {
                match ours.get(&key) {
                    Some(seen) if *seen != entry => {
                        return Err(format!(
                            "{origin}: {extension} {key} differs from {first_origin}"
                        ));
                    }
                    Some(_) => {}
                    None if key.starts_with("os:") => {
                        ours.insert(key, entry);
                        per_host.insert(extension.clone());
                    }
                    None => return Err(format!("{origin}: {extension} has a new key {key}")),
                }
            }
        }
    }
    if per_host.is_empty() {
        return Ok(first.clone());
    }
    let mut text: Vec<String> = first.lines().map(str::to_owned).collect();
    for extension in &per_host {
        let mut blocks: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        let mut body = 0..0;
        for (index, (origin, lock)) in locks.iter().enumerate() {
            let lines: Vec<&str> = if index == 0 {
                text.iter().map(String::as_str).collect()
            } else {
                lock.lines().collect()
            };
            let (range, entries) = extension_entries(&lines, extension)
                .map_err(|error| format!("{origin}: {error}"))?;
            if index == 0 {
                body = range;
            }
            for (key, block) in entries {
                blocks
                    .entry(key)
                    .or_insert_with(|| block.iter().map(|line| (*line).to_owned()).collect());
            }
        }
        let count = blocks.len();
        let mut replacement = Vec::new();
        for (position, (_, mut block)) in blocks.into_iter().enumerate() {
            if position + 1 < count
                && let Some(last) = block.last_mut()
            {
                last.push(',');
            }
            replacement.extend(block);
        }
        text.splice(body, replacement);
    }
    let mut result = text.join("\n");
    result.push('\n');
    let mut expected = base;
    expected.insert(
        "moduleExtensions".to_owned(),
        serde_json::Value::Object(merged),
    );
    if parse_lock("the merged lock", &result)? != expected {
        return Err(
            "the merged lock does not match its parts; Bazel's lock layout changed".to_owned(),
        );
    }
    Ok(result)
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
        let lock = merge_locks(&texts).map_err(|error| format!("{stem} lock: {error}"))?;
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
    fn union_drops_ids_of_file_only_digests() {
        let merged = union_lists(&[
            list("file:///tmp/x/tool a\nid-1 a\n"),
            list("https://b y\nid-2 Y\n"),
        ])
        .unwrap();
        assert_eq!(merged, "https://b y\nid-2 y\n");
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

    /// A lock as Bazel writes it, with the per-host extension `@@py//:x.bzl%python` keyed by `host`.
    fn host_lock(host: &str, digest: &str) -> String {
        format!(
            "{{\n  \"lockFileVersion\": 26,\n  \"moduleExtensions\": {{\n    \"@@a//:a.bzl%a\": {{\n      \"general\": {{\n        \"d\": 1\n      }}\n    }},\n    \"@@py//:x.bzl%python\": {{\n      \"{host}\": {{\n        \"d\": \"{digest}\",\n        \"r\": {{\n          \"k\": 1\n        }}\n      }}\n    }}\n  }}\n}}\n"
        )
    }

    #[test]
    fn lock_merge_keeps_identical_locks() {
        let lock = host_lock("os:linux,arch:amd64", "l");
        assert_eq!(merge_locks(&[list(&lock), list(&lock)]).unwrap(), lock);
    }

    #[test]
    fn lock_merge_fails_when_shared_fields_differ() {
        assert!(merge_locks(&[list("{}"), list("{\"a\":1}")]).is_err());
        let error = merge_locks(&[
            list(&host_lock("os:linux,arch:amd64", "l")),
            list(&host_lock("os:linux,arch:amd64", "other")),
        ])
        .unwrap_err();
        assert!(error.contains("os:linux,arch:amd64"), "{error}");
    }

    #[test]
    fn lock_merge_keeps_each_hosts_entry_sorted() {
        let merged = merge_locks(&[
            list(&host_lock("os:windows,arch:amd64", "w")),
            list(&host_lock("os:linux,arch:amd64", "l")),
            list(&host_lock("os:osx,arch:aarch64", "m")),
        ])
        .unwrap();
        let linux = host_lock("os:linux,arch:amd64", "l");
        let (head, _) = linux.split_once("      \"os:linux").unwrap();
        let entry = |host: &str, digest: &str| {
            format!(
                "      \"{host}\": {{\n        \"d\": \"{digest}\",\n        \"r\": {{\n          \"k\": 1\n        }}\n      }}"
            )
        };
        let expected = format!(
            "{head}{},\n{},\n{}\n    }}\n  }}\n}}\n",
            entry("os:linux,arch:amd64", "l"),
            entry("os:osx,arch:aarch64", "m"),
            entry("os:windows,arch:amd64", "w"),
        );
        assert_eq!(merged, expected);
    }

    #[test]
    fn lock_merges_crlf_only_difference_as_lf() {
        let unix = "{\n  \"f\": \"a\\nb\\n\"\n}\n";
        let windows = "{\r\n  \"f\": \"a\\r\\nb\\r\\n\"\r\n}\r\n";
        assert_eq!(merge_locks(&[list(windows), list(unix)]).unwrap(), unix);
        assert_eq!(merge_locks(&[list(unix), list(windows)]).unwrap(), unix);
    }

    #[test]
    fn lock_fails_on_real_difference_beside_crlf() {
        let unix = "{\n  \"f\": \"a\\nb\\n\"\n}\n";
        let windows = "{\r\n  \"f\": \"a\\r\\nc\\r\\n\"\r\n}\r\n";
        assert!(merge_locks(&[list(unix), list(windows)]).is_err());
    }
}
