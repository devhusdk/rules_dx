use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
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

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
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
    let toml = std::fs::read_to_string(workspace_root().join("clippy.toml"))
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
    rust_sources(&workspace_root(), &mut sources);
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
    assert!(
        roots >= 40,
        "the crate root sweep found {roots} roots; its data list stopped reaching them",
    );
}

#[test]
fn every_denied_lint_list_matches_the_documented_policy() {
    let policy = policy_lints();
    let mut sources = Vec::new();
    rust_sources(&workspace_root(), &mut sources);
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
        annotated >= 40,
        "the lint sweep found {annotated} annotated sources; its data list stopped reaching them",
    );
}
