use std::path::{Path, PathBuf};

const NEEDLE: &str = "Command::new(\"git\")";

const ROOTS: &[&str] = &["cli", "deploy"];

/// Directory prefixes end with `/`; other entries name one file.
const ALLOWED: &[&str] = &[
    "cli/process/",
    "cli/test_scratch/src/lib.rs",
];

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read_dir {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn is_allowed(rel: &str) -> bool {
    ALLOWED.iter().any(|entry| {
        if entry.ends_with('/') {
            rel.starts_with(entry)
        } else {
            rel == *entry
        }
    })
}

#[test]
fn git_is_spawned_only_through_the_dx_process_launcher() {
    let root = dx_testing::rules_dx_source();
    let mut sources = Vec::new();
    for dir in ROOTS {
        rust_sources(&root.join(dir), &mut sources);
    }
    sources.sort();
    assert!(!sources.is_empty(), "no Rust sources under {ROOTS:?}");

    let mut hits = Vec::new();
    for path in &sources {
        let rel = path
            .strip_prefix(&root)
            .expect("source under the checkout")
            .to_string_lossy()
            .replace('\\', "/");
        if is_allowed(&rel) {
            continue;
        }
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {rel}: {error}"));
        for (index, line) in text.lines().enumerate() {
            if line.contains(NEEDLE) {
                hits.push(format!("{rel}:{}", index + 1));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "direct {NEEDLE} outside the allowlist; use dx_process::capture::git instead:\n{}",
        hits.join("\n"),
    );
}

#[test]
fn every_allowlist_entry_still_spawns_git() {
    let root = dx_testing::rules_dx_source();
    let mut stale = Vec::new();
    for entry in ALLOWED {
        let path = root.join(entry);
        let mut sources = Vec::new();
        if path.is_dir() {
            rust_sources(&path, &mut sources);
        } else if path.is_file() {
            sources.push(path);
        }
        let spawns = sources.iter().any(|source| {
            std::fs::read_to_string(source)
                .unwrap_or_else(|error| panic!("read {}: {error}", source.display()))
                .contains(NEEDLE)
        });
        if !spawns {
            stale.push(*entry);
        }
    }
    assert!(stale.is_empty(), "drop these from ALLOWED: {stale:?}");
}
