//! No non-test source under `DIRS` spells a `@rules_dx//` target, or a target dx runs without the repo; those come from `dx_families`.

use std::collections::{BTreeMap, BTreeSet};

use starlark_syntax as _;
use yaml_serde as _;

/// Fewer snapshot entries than this under the checked dirs means the sweep lost the tree.
const MIN_RUST: usize = 100;

const DIRS: [&str; 17] = [
    "cli/update/",
    "cli/cli/",
    "cli/audit/",
    "cli/exec_audit/",
    "cli/exec_bench/",
    "cli/exec_deploy/",
    "cli/exec_docs/",
    "cli/exec_test_reports/",
    "cli/exec_update/",
    "cli/exec_worktree/",
    "cli/exec_quality/",
    "cli/plan/",
    "cli/ide/",
    "cli/shim/",
    "cli/adopt/",
    "cli/env_plan/",
    "cli/codegen/",
];

/// `(path, label)` pairs that may stay inline.
const EXEMPT: [(&str, &str); 2] = [
    // The shim does not link dx_families; its test pins the label to `dx_families::DX`.
    ("cli/shim/src/run.rs", "@rules_dx//:dx"),
    // A toolchain written into the consumer's MODULE.bazel, not a target dx runs.
    (
        "cli/adopt/src/lock_hubs.rs",
        "@rules_dx//rust:build_script_cc_env",
    ),
];

/// The repo-relative spelling of every `dx_families` command label but `:dx`, quoted.
fn bare_labels() -> Vec<String> {
    dx_families::command_labels()
        .filter(|label| *label != dx_families::DX)
        .map(|label| format!("\"{}\"", dx_families::repo_relative(label)))
        .collect()
}

/// The source up to its first `#[cfg(test)]` line.
fn non_test(source: &str) -> &str {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        if line.trim_start().starts_with("#[cfg(test)]") {
            return source.get(..offset).expect("string slice within bounds");
        }
        offset += line.len();
    }
    source
}

fn checked(path: &str) -> bool {
    DIRS.iter().any(|dir| path.starts_with(dir))
        && !path.ends_with("_test.rs")
        && !path.ends_with("_tests.rs")
        && !path.ends_with("/tests.rs")
        && !path.contains("/tests/")
}

/// The `path:line` of every string literal that starts with `@rules_dx//` and is not a `.bzl` label,
/// or that is a command label without `@rules_dx//`.
fn violations(rust: &BTreeMap<String, String>) -> BTreeSet<String> {
    let bare = bare_labels();
    let mut out = BTreeSet::new();
    for (path, source) in rust.iter().filter(|(path, _)| checked(path)) {
        for (index, line) in non_test(source).lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for (at, _) in line.match_indices("\"@rules_dx//") {
                let rest = line.get(at + 1..).expect("string slice within bounds");
                let literal = rest.split(['"', '\\']).next().unwrap_or(rest);
                if !literal.ends_with(".bzl") && !EXEMPT.contains(&(path.as_str(), literal)) {
                    out.insert(format!("{path}:{} {literal}", index + 1));
                }
            }
            for quoted in bare.iter().filter(|quoted| line.contains(quoted.as_str())) {
                out.insert(format!("{path}:{} {}", index + 1, quoted.trim_matches('"')));
            }
        }
    }
    out
}

fn read_rust() -> BTreeMap<String, String> {
    let key = std::env::var("DX_RUST_JSON").expect("DX_RUST_JSON is set by the test");
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("DX_RUST_JSON: {error}"));
    serde_json::from_str(&dx_testing::read_text(&path))
        .unwrap_or_else(|error| panic!("DX_RUST_JSON: {error}"))
}

/// The `BUILD.bazel` path of the package `label` names.
fn build_file(label: &str) -> String {
    let package = dx_families::repo_relative(label)
        .split(':')
        .next()
        .unwrap_or_default();
    if package.is_empty() {
        "BUILD.bazel".to_owned()
    } else {
        format!("{package}/BUILD.bazel")
    }
}

fn texts(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn the_tree_spells_no_command_label() {
    let rust = read_rust();
    let count = rust.keys().filter(|path| checked(path)).count();
    assert!(
        count >= MIN_RUST,
        "only {count} non-test .rs files under {DIRS:?} in the rust snapshot"
    );
    let found = violations(&rust);
    assert!(
        found.is_empty(),
        "inline @rules_dx command labels; take them from dx_families:\n{}",
        found.into_iter().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn every_command_label_names_a_package() {
    let key = std::env::var("DX_SOURCES_JSON").expect("DX_SOURCES_JSON is set by the test");
    let path =
        dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("DX_SOURCES_JSON: {error}"));
    let sources: BTreeMap<String, String> = serde_json::from_str(&dx_testing::read_text(&path))
        .unwrap_or_else(|error| panic!("DX_SOURCES_JSON: {error}"));
    let missing: BTreeSet<&str> = dx_families::command_labels()
        .filter(|label| !sources.contains_key(&build_file(label)))
        .collect();
    assert!(
        missing.is_empty(),
        "dx_families command labels name packages with no BUILD.bazel:\n{}",
        missing.into_iter().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn build_file_names_the_package_build_file() {
    assert_eq!(build_file(dx_families::DX), "BUILD.bazel");
    assert_eq!(build_file("@rules_dx//dx:env"), "dx/BUILD.bazel");
    assert_eq!(build_file("@rules_dx//dx/x"), "dx/x/BUILD.bazel");
}

#[test]
fn a_command_label_is_a_violation() {
    let rust = texts(&[(
        "cli/update/src/backend.rs",
        "let a = [\"@rules_dx//go/private/tools:go\", \"--\"];\n",
    )]);
    assert_eq!(
        violations(&rust),
        BTreeSet::from(["cli/update/src/backend.rs:1 @rules_dx//go/private/tools:go".to_owned()])
    );
}

#[test]
fn a_bare_command_label_is_a_violation() {
    let rust = texts(&[
        (
            "cli/ide/src/go.rs",
            "commands: &[\"go/private/ide:gopackagesdriver\"],\n",
        ),
        ("cli/env_plan/src/lib.rs", "const T: &str = \"dx:env\";\n"),
    ]);
    assert_eq!(
        violations(&rust),
        BTreeSet::from([
            "cli/env_plan/src/lib.rs:1 dx:env".to_owned(),
            "cli/ide/src/go.rs:1 go/private/ide:gopackagesdriver".to_owned(),
        ])
    );
}

#[test]
fn bzl_labels_tests_comments_and_other_dirs_pass() {
    let rust = texts(&[
        (
            "cli/cli/src/a.rs",
            "let b = \"@rules_dx//go:extensions.bzl\";\n// \"@rules_dx//:dx\"\n#[cfg(test)]\nmod tests {\n    const X: &str = \"@rules_dx//:dx\";\n}\n",
        ),
        (
            "cli/cli/src/exec/audit_live_tests.rs",
            "\"@rules_dx//:dx\"\n",
        ),
        ("cli/deploy/src/plan.rs", "\"@rules_dx//:dx\"\n"),
        (
            "cli/adopt/src/licenses.rs",
            "\"use_extension(\\\"@rules_dx//core:extensions.bzl\\\", \\\"x\\\")\"\n",
        ),
        (
            "cli/shim/src/run.rs",
            "const DX: &str = \"@rules_dx//:dx\";\n",
        ),
        (
            "cli/audit/src/a.rs",
            "format!(\"bazel run @rules_dx//:dx\")\n",
        ),
    ]);
    assert!(violations(&rust).is_empty(), "{:?}", violations(&rust));
}
