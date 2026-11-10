//! What every quality run loads, whatever languages the workspace has, reaches no language rule set.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The only external repositories the core load graph may load from.
const ALLOWED_REPOS: &[&str] = &[
    "bazel_features",
    "bazel_lib",
    "bazel_skylib",
    "bazel_tools",
    "dx_pins_data",
    "dx_prebuilt",
    "platforms",
    "rules_dx",
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

/// Directories outside `core/quality/` whose `.bzl` files are roots.
const CORE_DIRS: &[&str] = &["core", "core/codegen", "core/modules"];

/// Core files outside `core/quality/` that load a language rule set; this list only shrinks.
const PENDING_CORE_FILES: &[&str] = &[
    "core/native_coverage.bzl", // #10085
];

/// Packages of core tool defaults that load a language rule set; this list only shrinks.
const PENDING_TOOL_PACKAGES: &[&str] = &[
    "core/quality/tools/python",     // #10545
    "core/quality/tools/nixfmt/bin", // #10565
    "rust/private/quality/tools",    // #10567
];

/// Tier packages that load a language rule set; this list only shrinks.
const PENDING_TIER_PACKAGES: &[&str] = &[
    "core/quality/tiers/gopls",    // #10622
    "core/quality/tiers/prettier", // #10622
];

/// Top-level packages `core/` may load from or point labels into.
const CORE_PACKAGES: &[&str] = &["cli", "config", "core", "tools"];

/// Files whose `//` loads and labels may still reach a language package; this list only shrinks.
const PENDING_LANGUAGE_REACH: &[&str] = &[
    "core/quality/families/core_probe.bzl",
    "core/quality/fixtures.bzl",
    "core/quality/tiers/gopls/BUILD.bazel",
    "core/quality/tools.bzl",
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

/// The `BUILD.bazel` paths of the tier packages that do not load a language rule set yet.
fn tier_packages(source: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(source.join("core/quality/tiers"))
        .expect("tiers dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("BUILD.bazel").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .map(|name| format!("core/quality/tiers/{name}"))
        .filter(|package| !PENDING_TIER_PACKAGES.contains(&package.as_str()))
        .map(|package| format!("{package}/BUILD.bazel"))
        .collect();
    found.sort();
    found
}

/// Quoted `//pkg:name` labels in `load(` and `Label(` calls of `text`, without any repo prefix.
fn loaded_labels(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for marker in ["load(\"", "Label(\""] {
        for part in text.split(marker).skip(1) {
            if let Some(label) = part.split('"').next() {
                let label = label.strip_prefix("@rules_dx").unwrap_or(label);
                if label.starts_with("//") {
                    found.push(label.to_owned());
                }
            }
        }
    }
    found
}

/// Whether `label` names a language package or any `private/` package outside `core/`.
fn reaches_language(label: &str) -> bool {
    let package = label
        .trim_start_matches("//")
        .split(':')
        .next()
        .unwrap_or_default();
    let top = package.split('/').next().unwrap_or_default();
    if top.is_empty() {
        return false;
    }
    if top == "core" {
        return false;
    }
    !CORE_PACKAGES.contains(&top) || package.split('/').any(|part| part == "private")
}

fn roots(source: &Path) -> Vec<String> {
    let mut roots = bzl_files(&source.join("config"), "config");
    roots.push("config/BUILD.bazel".to_owned());
    for dir in CORE_DIRS {
        roots.extend(
            bzl_files(&source.join(dir), dir)
                .into_iter()
                .filter(|path| {
                    !path.ends_with("_tests.bzl") && !PENDING_CORE_FILES.contains(&path.as_str())
                }),
        );
    }
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
    roots.extend(tier_packages(source));
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
fn pending_core_files_still_offend() {
    let source = dx_testing::rules_dx_source();
    for path in PENDING_CORE_FILES {
        let found = dx_adopt::external_repos(&source, &[(*path).to_owned()]).expect("load graph");
        assert!(
            found
                .values()
                .flatten()
                .any(|repo| !ALLOWED_REPOS.contains(&repo.as_str())),
            "{path} reaches no language rule set; drop it from PENDING_CORE_FILES"
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

#[test]
fn core_reaches_no_language_package() {
    let source = dx_testing::rules_dx_source();
    let mut offenders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut paths = roots(&source);
    paths.extend(
        PENDING_TIER_PACKAGES
            .iter()
            .map(|package| format!("{package}/BUILD.bazel")),
    );
    for path in paths {
        let bad: Vec<String> = loaded_labels(&dx_testing::read_text(&source.join(&path)))
            .into_iter()
            .filter(|label| reaches_language(label))
            .collect();
        if !bad.is_empty() && !PENDING_LANGUAGE_REACH.contains(&path.as_str()) {
            offenders.insert(path, bad);
        }
    }
    assert!(
        offenders.is_empty(),
        "core loads or labels reach a language package or a private/ package:\n{offenders:#?}"
    );
}

#[test]
fn pending_language_reach_still_offends() {
    let source = dx_testing::rules_dx_source();
    for path in PENDING_LANGUAGE_REACH {
        let found = loaded_labels(&dx_testing::read_text(&source.join(path)));
        assert!(
            found.iter().any(|label| reaches_language(label)),
            "{path} reaches no language package; drop it from PENDING_LANGUAGE_REACH"
        );
    }
}

#[test]
fn pending_tier_packages_still_offend() {
    let source = dx_testing::rules_dx_source();
    for package in PENDING_TIER_PACKAGES {
        let root = format!("{package}/BUILD.bazel");
        let found = dx_adopt::external_repos(&source, &[root]).expect("load graph");
        assert!(
            found
                .values()
                .flatten()
                .any(|repo| !ALLOWED_REPOS.contains(&repo.as_str())),
            "{package} reaches no language rule set; drop it from PENDING_TIER_PACKAGES"
        );
    }
}
