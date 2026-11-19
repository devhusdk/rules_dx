use std::collections::BTreeSet;

fn read(key: &str) -> String {
    let rel = std::env::var(key).unwrap_or_else(|_| panic!("{key} must name a file"));
    let path = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn every_listed_file_was_followed_without_reaching_a_dev_only_repository() {
    let listed = lines(&read("DX_SURFACE_LIST"));
    let closure = lines(&read("DX_SURFACE_CLOSURE"));
    for label in &listed {
        assert!(
            closure.contains(label),
            "{label} missing from the load closure"
        );
    }
}

/// Returns `<dir>:<file>` for each tracked top-level `<dir>/<file>`.
fn top_level(paths: &[String], file: &str) -> BTreeSet<String> {
    let suffix = format!("/{file}");
    paths
        .iter()
        .filter_map(|path| path.strip_suffix(&suffix))
        .filter(|dir| !dir.contains('/'))
        .map(|dir| format!("{dir}:{file}"))
        .collect()
}

#[test]
fn public_api_snapshot_is_in_the_consumer_surface() {
    let snapshot: dx_testing::serde_json::Value =
        dx_testing::serde_json::from_str(&read("DX_API_STARLARK"))
            .expect("api/starlark.json is JSON");
    let entries = snapshot.as_array().expect("api/starlark.json is an array");
    assert!(!entries.is_empty(), "api/starlark.json lists no files");
    let listed = lines(&read("DX_SURFACE_LIST"));
    for entry in entries {
        let load = entry["load"]
            .as_str()
            .expect("each entry has a load string");
        let label = load
            .strip_prefix("@rules_dx//")
            .unwrap_or_else(|| panic!("{load} is not a @rules_dx// label"));
        assert!(
            listed.contains(label),
            "{label} missing from the generated consumer surface; list it in _STARLARK_SRCS in tools/api/BUILD.bazel and run bazel run //tools/api:update"
        );
    }
}

#[test]
fn entry_points_are_listed_in_the_consumer_surface() {
    let paths = dx_testing::source_paths();
    let languages: BTreeSet<String> = top_level(&paths, "lang.json")
        .into_iter()
        .filter_map(|label| label.strip_suffix(":lang.json").map(str::to_string))
        .collect();
    let mut entry: BTreeSet<String> = top_level(&paths, "defs.bzl")
        .into_iter()
        .filter(|label| {
            label
                .split_once(':')
                .is_some_and(|(dir, _)| languages.contains(dir))
        })
        .collect();
    assert!(!entry.is_empty(), "the snapshot holds no <lang>/defs.bzl");
    entry.extend(top_level(&paths, "extensions.bzl"));
    let listed = lines(&read("DX_SURFACE_LIST"));
    for label in &entry {
        assert!(
            listed.contains(label),
            "{label} missing from the generated consumer surface; list it in _STARLARK_SRCS in tools/api/BUILD.bazel and run bazel run //tools/api:update"
        );
    }
    for label in &listed {
        assert!(
            !label.contains("/rules:"),
            "{label}: the entry point is <lang>/defs.bzl"
        );
    }
}

#[test]
fn no_tracked_rules_defs_bzl() {
    let offenders: Vec<String> = dx_testing::source_paths()
        .iter()
        .filter(|path| path.as_str() == "rules/defs.bzl" || path.ends_with("/rules/defs.bzl"))
        .cloned()
        .collect();
    assert!(
        offenders.is_empty(),
        "tracked rules/defs.bzl: {offenders:?}"
    );
}
