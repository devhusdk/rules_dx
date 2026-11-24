use std::collections::BTreeSet;

use dx_guard::masked::masked;
use dx_guard::snapshot::tree;
use serde_json as _;
use starlark_syntax as _;
use yaml_serde as _;

const LAYOUT_DIRS: [&str; 4] = ["private", "tests", "tools", "third_party"];

const PUBLIC_NAMES: [&str; 2] = ["defs.bzl", "extensions.bzl"];

const WORKSPACE_MARKERS: [&str; 4] = ["MODULE.bazel", "REPO.bazel", "WORKSPACE", "WORKSPACE.bazel"];

const MIN_FILES: usize = 150;

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn workspace_roots(files: &[(String, String)]) -> BTreeSet<&str> {
    files
        .iter()
        .map(|(path, _)| path.as_str())
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            WORKSPACE_MARKERS.contains(&name) || name.ends_with(".MODULE.bazel")
        })
        .map(dir_of)
        .filter(|dir| !dir.is_empty())
        .collect()
}

fn needs_declaration(path: &str, roots: &BTreeSet<&str>) -> bool {
    let (dirs, name) = path.rsplit_once('/').unwrap_or(("", path));
    if !name.ends_with(".bzl") || PUBLIC_NAMES.contains(&name) {
        return false;
    }
    if dirs.split('/').any(|part| LAYOUT_DIRS.contains(&part)) {
        return false;
    }
    !roots
        .iter()
        .any(|root| path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'/'))
}

fn declares_visibility(text: &str) -> bool {
    masked(text)
        .mask
        .lines()
        .any(|line| line.starts_with("visibility("))
}

fn undeclared(files: &[(String, String)]) -> (usize, Vec<&str>) {
    let roots = workspace_roots(files);
    let mut checked = 0;
    let mut missing = Vec::new();
    for (path, text) in files {
        if !needs_declaration(path, &roots) {
            continue;
        }
        checked += 1;
        if !declares_visibility(text) {
            missing.push(path.as_str());
        }
    }
    (checked, missing)
}

#[test]
fn every_internal_bzl_declares_its_load_visibility() {
    let root = dx_testing::source_snapshot();
    let files = tree(&root).unwrap_or_else(|error| panic!("{}: {error}", root.display()));
    let (checked, missing) = undeclared(&files);
    assert!(
        checked >= MIN_FILES,
        "only {checked} internal .bzl files in the snapshot"
    );
    assert!(
        missing.is_empty(),
        "these .bzl files have no top-level visibility() call. Add visibility(\"private\") when only \
         their own package loads them, visibility([\"//<dir>/...\", ...]) naming the packages that do, or \
         visibility(\"public\") when code outside rules_dx loads them:\n{}",
        missing.join("\n")
    );
}

fn fixture(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn an_internal_file_without_a_declaration_is_named() {
    let files = fixture(&[
        ("core/x.bzl", "\"\"\"Doc.\"\"\"\n\nX = 1\n"),
        (
            "core/y.bzl",
            "load(\":x.bzl\", \"X\")\n\nvisibility(\"private\")\n",
        ),
        (
            "core/z.bzl",
            "visibility([\"//core/...\", \"//rust/...\"])\n",
        ),
    ]);
    let (checked, missing) = undeclared(&files);
    assert_eq!(checked, 3);
    assert_eq!(missing, ["core/x.bzl"]);
}

#[test]
fn a_commented_or_nested_call_does_not_count() {
    let files = fixture(&[
        ("core/a.bzl", "# visibility(\"private\")\n"),
        ("core/b.bzl", "def f():\n    visibility(\"private\")\n"),
        (
            "core/c.bzl",
            "X = \"\"\"\nvisibility(\"private\")\n\"\"\"\n",
        ),
    ]);
    let (_, missing) = undeclared(&files);
    assert_eq!(missing, ["core/a.bzl", "core/b.bzl", "core/c.bzl"]);
}

#[test]
fn public_names_layout_dirs_and_fixture_workspaces_are_not_checked() {
    let files = fixture(&[
        ("rust/defs.bzl", ""),
        ("rust/extensions.bzl", ""),
        ("rust/private/rustc.bzl", ""),
        ("rust/tests/defs_tests.bzl", ""),
        ("tools/ci/repos.bzl", ""),
        ("third_party/x/y.bzl", ""),
        ("bcr_test_module/MODULE.bazel", ""),
        ("bcr_test_module/toolchain.bzl", ""),
        ("cc/fixture/consumer.MODULE.bazel", ""),
        ("cc/fixture/sub/x.bzl", ""),
        ("core/BUILD.bazel", ""),
    ]);
    let (checked, _) = undeclared(&files);
    assert_eq!(checked, 0);
}
