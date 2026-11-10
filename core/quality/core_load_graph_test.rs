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

/// Language helpers that sit in `core/quality/` and are not roots; this list only shrinks.
const LANGUAGE_HELPERS: &[&str] = &[
    "elixirc",
    "erlc",
    "fsc",
    "jvm_check",
    "nvcc_syntax",
    "roslyn",
];

/// Packages of core tool defaults that load a language rule set; this list only shrinks.
const PENDING_TOOL_PACKAGES: &[&str] = &[
    "core/quality/tools/python",     // #10545
    "haskell/private/tools/hlint",   // #10548
    "haskell/private/tools/ormolu",  // #10548
    "haskell/private/tools/weeder",  // #10548
    "core/quality/tools/nixfmt/bin", // #10565
    "core/quality/tools/xml",        // #10566
    "rust/private/quality/tools",    // #10567
];

/// External repositories named by `"@repo//` labels in `actual =` and `select()` values of `text`.
fn alias_repos(text: &str) -> BTreeSet<String> {
    text.split("\"@")
        .skip(1)
        .filter_map(|part| part.split_once("//").map(|(repo, _)| repo))
        .filter(|repo| {
            !repo.is_empty()
                && repo
                    .chars()
                    .all(|c| c.is_alphanumeric() || "_.-+~".contains(c))
        })
        .map(str::to_owned)
        .collect()
}

/// Quoted `//pkg:name` strings that follow `default = ` or `Label(` in `text`.
fn default_labels(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for marker in ["default = \"", "Label(\""] {
        for part in text.split(marker).skip(1) {
            if let Some(label) = part.split('"').next() {
                if label.starts_with("//") {
                    found.push(label.to_owned());
                }
            }
        }
    }
    found
}

/// The `BUILD.bazel` paths of the packages the core tool defaults point into.
fn tool_packages(source: &Path) -> Vec<String> {
    let labels = dx_testing::read_text(&source.join("core/quality/real_aspect_labels.bzl"));
    let mut files: BTreeSet<String> = labels
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('"')?
                .split('%')
                .next()?
                .split_once(':')
        })
        .map(|(package, file)| format!("{package}/{file}"))
        .filter(|path| path.ends_with(".bzl"))
        .collect();
    files.insert("core/quality/families/core.bzl".to_owned());
    let config = dx_testing::read_text(&source.join("config/BUILD.bazel"));
    let mut packages = BTreeSet::new();
    for file in &files {
        for label in default_labels(&dx_testing::read_text(&source.join(file))) {
            let mut label = label;
            if let Some(name) = label.strip_prefix("//config:tool_") {
                let block = config
                    .split("name = \"tool_")
                    .skip(1)
                    .find(|block| block.starts_with(&format!("{name}\"")));
                let default = block
                    .and_then(|block| block.split("build_setting_default = \"").nth(1))
                    .and_then(|rest| rest.split('"').next());
                match default {
                    Some(default) if !default.starts_with('@') => {
                        label = match default.strip_prefix(':') {
                            Some(target) => format!("//config:{target}"),
                            None => default.to_owned(),
                        };
                    }
                    _ => continue,
                }
            }
            if let Some(rest) = label.strip_prefix("//") {
                let package = rest.split(':').next().unwrap_or(rest);
                packages.insert(package.to_owned());
            }
        }
    }
    packages
        .into_iter()
        .map(|package| format!("{package}/BUILD.bazel"))
        .collect()
}

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
        bzl_files(&source.join("core/quality"), "core/quality")
            .into_iter()
            .filter(|path| {
                let stem = path
                    .trim_start_matches("core/quality/")
                    .trim_end_matches(".bzl");
                !stem.ends_with("_tests") && !LANGUAGE_HELPERS.contains(&stem)
            }),
    );
    roots.extend(
        tool_packages(source)
            .into_iter()
            .filter(|path| !PENDING_TOOL_PACKAGES.contains(&path.trim_end_matches("/BUILD.bazel"))),
    );
    roots.push("core/quality/families/core.bzl".to_owned());
    roots.push("core/quality/families/core_probe.bzl".to_owned());
    roots.push("tools/docs/aspect.bzl".to_owned());
    roots
}

#[test]
fn core_loads_only_allowed_external_repos() {
    let source = dx_testing::rules_dx_source();
    let roots = roots(&source);
    let found = dx_adopt::external_repos(&source, &roots).expect("load graph");
    let mut aliased: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in roots
        .iter()
        .filter(|path| path.ends_with("/BUILD.bazel") && *path != "config/BUILD.bazel")
    {
        let bad: BTreeSet<String> = alias_repos(&dx_testing::read_text(&source.join(path)))
            .into_iter()
            .filter(|repo| repo != "dx_tools" && !ALLOWED_REPOS.contains(&repo.as_str()))
            .collect();
        if !bad.is_empty() {
            aliased.insert(path.clone(), bad);
        }
    }
    assert!(
        aliased.is_empty(),
        "core tool defaults alias into source-built external repos; use a pinned prebuilt in @dx_tools:\n{aliased:#?}"
    );
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
            source.join(format!("core/quality/{helper}.bzl")).is_file(),
            "core/quality/{helper}.bzl is gone; drop it from LANGUAGE_HELPERS"
        );
    }
}

#[test]
fn pending_tool_packages_still_offend() {
    let source = dx_testing::rules_dx_source();
    for package in PENDING_TOOL_PACKAGES {
        let root = format!("{package}/BUILD.bazel");
        let found = dx_adopt::external_repos(&source, &[root.clone()]).expect("load graph");
        let offends = found
            .values()
            .flatten()
            .any(|repo| !ALLOWED_REPOS.contains(&repo.as_str()))
            || alias_repos(&dx_testing::read_text(&source.join(&root)))
                .iter()
                .any(|repo| repo != "dx_tools" && !ALLOWED_REPOS.contains(&repo.as_str()));
        assert!(
            offends,
            "{package} reaches no language rule set; drop it from PENDING_TOOL_PACKAGES"
        );
    }
}
