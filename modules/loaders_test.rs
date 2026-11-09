//! Every non-dev bazel_dep of the root module is loaded by a public .bzl.

use std::path::Path;

use dx_module_bazel::{ModuleFile, Value};

const SKIPPED_DIRS: &[&str] = &[
    "tests",
    "test",
    "testdata",
    "fixtures",
    "examples",
    "e2e",
    "testing",
    "node_modules",
];

const SKIPPED_ROOTS: &[&str] = &["tools", "docs", "bcr_test_module"];

fn public_bzl(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read_dir {dir:?}: {error}"));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.starts_with("bazel-") {
            continue;
        }
        if path.is_dir() {
            let top = dir == root;
            if SKIPPED_DIRS.contains(&name.as_str())
                || (top && SKIPPED_ROOTS.contains(&name.as_str()))
                || path.join("MODULE.bazel").exists()
            {
                continue;
            }
            public_bzl(root, &path, found);
        } else if name.ends_with(".bzl") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                found.push(text);
            }
        }
    }
}

fn mentions(text: &str, dep: &str) -> bool {
    let needle = format!("@{dep}");
    text.match_indices(&needle).any(|(start, found)| {
        matches!(
            text[start + found.len()..].chars().next(),
            Some('/' | '"' | ':' | '+')
        )
    })
}

fn non_dev_deps(source: &str) -> Vec<String> {
    let module = ModuleFile::parse(source).unwrap_or_else(|error| panic!("{error}"));
    module
        .calls_named("bazel_dep")
        .filter(|call| !matches!(call.arg("dev_dependency"), Some(Value::Ident(value)) if value == "True"))
        .filter_map(|call| call.str("name").map(str::to_owned))
        .collect()
}

/// The deps in `deps` that no text in `bzl` mentions.
fn unloaded(deps: &[String], bzl: &[String]) -> Vec<String> {
    deps.iter()
        .filter(|dep| !bzl.iter().any(|text| mentions(text, dep)))
        .map(|dep| {
            format!("bazel_dep {dep:?} is loaded by no public .bzl; make it dev_dependency or move it")
        })
        .collect()
}

#[test]
fn every_non_dev_bazel_dep_is_loaded_by_a_public_bzl() {
    let root = dx_testing::rules_dx_source();
    let mut bzl = Vec::new();
    public_bzl(&root, &root, &mut bzl);
    assert!(
        bzl.len() > 100,
        "found only {} public .bzl files",
        bzl.len()
    );
    let deps = non_dev_deps(&dx_testing::read_runfiles("MODULE.bazel"));
    let problems = unloaded(&deps, &bzl);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn an_unloaded_dep_fails() {
    let deps = vec!["demo".to_owned(), "used".to_owned()];
    let bzl = vec!["load(\"@used//:defs.bzl\", \"x\")".to_owned()];
    let problems = unloaded(&deps, &bzl);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("\"demo\""), "{problems:?}");
}

#[test]
fn a_prefix_of_another_repo_name_is_not_a_load() {
    assert!(!mentions(
        "load(\"@rules_python_extra//:x.bzl\")",
        "rules_python"
    ));
    assert!(mentions("load(\"@rules_python//:x.bzl\")", "rules_python"));
    assert!(mentions("\"@rules_python\"", "rules_python"));
}
