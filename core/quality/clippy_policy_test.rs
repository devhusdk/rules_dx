use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dx_testing::{rules_dx_source, runfiles_root};

fn registered_crate_roots() -> usize {
    std::env::var("DX_CRATE_ROOTS")
        .expect("DX_CRATE_ROOTS must list the registered crate roots")
        .split_whitespace()
        .filter(|rel| is_crate_root(rel))
        .count()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("readable runfiles dir") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn is_skipped_dir(rel: &str, name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with("bazel-")
        || name == "node_modules"
        || name == "fixtures"
        || name == "testdata"
        || rel == "examples"
}

fn checkout_crate_roots(dir: &Path, rel: &str, out: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).expect("readable checkout dir") {
        let entry = entry.expect("readable dir entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        let kind = entry.file_type().expect("readable file type");
        if kind.is_dir() {
            if !is_skipped_dir(&child, &name) {
                checkout_crate_roots(&entry.path(), &child, out);
            }
        } else if kind.is_file() && is_crate_root(&child) {
            out.insert(child);
        }
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(runfiles_root())
        .expect("runfiles path under the workspace")
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_crate_root(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() < 3 || parts[parts.len() - 2] != "src" {
        return false;
    }
    let file = parts[parts.len() - 1];
    file == "lib.rs" || file == "main.rs" || (file.starts_with("bin_") && file.ends_with(".rs"))
}

fn squeezed(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn denied_lints(text: &str) -> Option<BTreeSet<String>> {
    let squeezed = squeezed(text);
    let open = "#![cfg_attr(not(test),deny(";
    let start = squeezed.find(open)? + open.len();
    let rest = &squeezed[start..];
    let end = rest.find("))]")?;
    Some(rest[..end].split(',').map(|lint| lint.to_owned()).collect())
}

fn policy_lints() -> BTreeSet<String> {
    let toml = std::fs::read_to_string(runfiles_root().join("tools/workspace/clippy.toml"))
        .expect("clippy.toml must ship as test data");
    let sentence = toml
        .lines()
        .find_map(|line| line.trim().strip_prefix('#'))
        .expect("clippy.toml documents its lint intent in a comment");
    let after = sentence
        .split_once("denies ")
        .unwrap_or_else(|| panic!("clippy.toml must name the denied lints: {sentence}"))
        .1;
    let named = after
        .split_once(" outside cfg(test).")
        .unwrap_or_else(|| {
            panic!("clippy.toml must scope the lints to non-test builds: {sentence}")
        })
        .0;
    named
        .replace(" and ", ", ")
        .split(',')
        .map(|lint| format!("clippy::{}", lint.trim()))
        .collect()
}

#[test]
fn every_crate_root_denies_the_lints_clippy_toml_documents() {
    let policy = policy_lints();
    assert_eq!(
        policy,
        [
            "clippy::expect_used",
            "clippy::todo",
            "clippy::unreachable",
            "clippy::unwrap_used",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "clippy.toml must name the four crate lints the repo denies",
    );

    let mut sources = Vec::new();
    rust_sources(&runfiles_root(), &mut sources);
    sources.sort();
    let mut roots = 0usize;
    for path in &sources {
        let rel = relative(path);
        if !is_crate_root(&rel) {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("readable rust source");
        roots += 1;
        assert_eq!(
            denied_lints(&text).as_ref(),
            Some(&policy),
            "{rel} must deny every lint clippy.toml documents outside cfg(test)",
        );
    }
    assert_eq!(
        roots,
        registered_crate_roots(),
        "the crate root sweep found {roots} roots; they must match \
         //core/quality:clippy_policy_test's crate root list"
    );
}

#[test]
fn every_denied_lint_list_matches_the_documented_policy() {
    let policy = policy_lints();
    let mut sources = Vec::new();
    rust_sources(&runfiles_root(), &mut sources);
    sources.sort();
    let mut annotated = 0usize;
    for path in &sources {
        let text = std::fs::read_to_string(path).expect("readable rust source");
        let Some(denied) = denied_lints(&text) else {
            continue;
        };
        annotated += 1;
        assert_eq!(
            denied,
            policy,
            "{} denies a different lint list than clippy.toml documents",
            relative(path),
        );
    }
    assert!(
        annotated >= registered_crate_roots(),
        "the lint sweep found {annotated} annotated sources; its data list stopped reaching them",
    );
}

#[test]
fn every_checkout_crate_root_is_registered() {
    let registered: BTreeSet<String> = std::env::var("DX_CRATE_ROOTS")
        .expect("DX_CRATE_ROOTS must list the registered crate roots")
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut found = BTreeSet::new();
    checkout_crate_roots(&rules_dx_source(), "", &mut found);
    let missing: Vec<&String> = found.difference(&registered).collect();
    assert!(
        missing.is_empty(),
        "crate roots missing from _CLIPPY_POLICY_CRATE_ROOTS in core/quality/BUILD.bazel: {missing:?}",
    );
}
