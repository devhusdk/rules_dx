//! No repos.bzl calls module_extension(): module extensions live in the sibling extensions.bzl.

use std::path::{Path, PathBuf};

use dx_testing::runfiles_root;

/// True when a line that is not a comment calls module_extension(.
fn calls_module_extension(source: &str) -> bool {
    source
        .lines()
        .map(str::trim_start)
        .any(|line| !line.starts_with('#') && line.contains("module_extension("))
}

/// Every repos.bzl under `dir`, sorted.
fn repos_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current)
            .unwrap_or_else(|error| panic!("read_dir {}: {error}", current.display()));
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().and_then(|name| name.to_str()) == Some("repos.bzl") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn module_extension_calls_are_detected() {
    assert!(calls_module_extension("ruby = module_extension(\n"));
    assert!(!calls_module_extension("# module_extension(\n"));
}

#[test]
fn repos_bzl_defines_no_module_extension() {
    let root = runfiles_root();
    let files = repos_files(&root);
    assert!(!files.is_empty(), "no repos.bzl in the runfiles");
    let offenders: Vec<String> = files
        .iter()
        .filter(|path| {
            let source = std::fs::read_to_string(path).expect("read repos.bzl");
            calls_module_extension(&source)
        })
        .map(|path| path.strip_prefix(&root).unwrap_or(path).display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "repos.bzl must not call module_extension(; move it to extensions.bzl:\n{}",
        offenders.join("\n")
    );
}
