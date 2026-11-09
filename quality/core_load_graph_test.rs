//! What every quality run loads, whatever languages the workspace has, reaches no language rule set.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The only external repositories the core load graph may load from.
const ALLOWED_REPOS: &[&str] = &[
    "bazel_features",
    "bazel_lib",
    "bazel_skylib",
    "bazel_tools",
    "dx_prebuilt",
    "platforms",
];

/// Language helpers that sit in `quality/` and are not roots; this list only shrinks.
const LANGUAGE_HELPERS: &[&str] = &[
    "clang_syntax",
    "elixirc",
    "erlc",
    "fsc",
    "go_analysis",
    "jvm_check",
    "nvcc_syntax",
    "roslyn",
    "tsc",
    "tsc_report",
];

fn bzl_files(dir: &Path, prefix: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{dir:?}: {error}"))
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".bzl"))
        .map(|name| format!("{prefix}/{name}"))
        .collect();
    found.sort();
    found
}

fn roots(source: &Path) -> Vec<String> {
    let mut roots = bzl_files(&source.join("config"), "config");
    roots.push("config/BUILD.bazel".to_owned());
    roots.extend(
        bzl_files(&source.join("quality"), "quality")
            .into_iter()
            .filter(|path| {
                let stem = path.trim_start_matches("quality/").trim_end_matches(".bzl");
                !stem.ends_with("_tests") && !LANGUAGE_HELPERS.contains(&stem)
            }),
    );
    roots.push("quality/families/core.bzl".to_owned());
    roots.push("quality/families/core_probe.bzl".to_owned());
    roots
}

#[test]
fn core_loads_only_allowed_external_repos() {
    let source = dx_testing::rules_dx_source();
    let roots = roots(&source);
    let found = dx_adopt::external_repos(&source, &roots).expect("load graph");
    let offenders: BTreeMap<&str, BTreeSet<&str>> = found
        .iter()
        .filter_map(|(file, repos)| {
            let bad: BTreeSet<&str> = repos
                .iter()
                .map(String::as_str)
                .filter(|repo| !ALLOWED_REPOS.contains(repo))
                .collect();
            (!bad.is_empty()).then_some((file.as_str(), bad))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "core loads reach a language rule set; move the load behind a language aspect file:\n{offenders:#?}"
    );
}

#[test]
fn language_helpers_exist() {
    let source = dx_testing::rules_dx_source();
    for helper in LANGUAGE_HELPERS {
        assert!(
            source.join(format!("quality/{helper}.bzl")).is_file(),
            "quality/{helper}.bzl is gone; drop it from LANGUAGE_HELPERS"
        );
    }
}
