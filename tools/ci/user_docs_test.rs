use std::path::{Component, Path, PathBuf};

use dx_testing::{read_runfiles, runfiles_root};

const INDEXES: [&str; 4] = [
    "README.md",
    "docs/README.md",
    "docs/cli/commands/README.md",
    "examples/README.md",
];

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn unique(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

/// Returns USER_PROSE from docs/site/BUILD.bazel as workspace-relative paths.
fn published() -> Vec<String> {
    let build = read_runfiles("docs/site/BUILD.bazel");
    let mut names = Vec::new();
    let mut inside = false;
    for line in build.lines() {
        let line = line.trim();
        if line.starts_with("USER_PROSE") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if line.starts_with(']') {
            break;
        }
        let label = line
            .trim_end_matches(',')
            .trim()
            .trim_matches('"')
            .to_owned();
        assert!(
            label.starts_with("//"),
            "USER_PROSE entry {label:?} is not a label"
        );
        let rest = label.trim_start_matches("/");
        let (package, file) = rest
            .split_once(':')
            .unwrap_or_else(|| panic!("USER_PROSE label {label} names no file"));
        let mut path = PathBuf::new();
        if !package.is_empty() {
            path.push(package);
        }
        path.push(file);
        names.push(path.to_string_lossy().replace('\\', "/"));
    }
    assert!(
        !names.is_empty(),
        "docs/site/BUILD.bazel ships no USER_PROSE"
    );
    unique(names)
}

/// Returns every Markdown link target in one page, in source order.
fn markdown_links(page: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = page;
    while let Some(start) = rest.find("](") {
        let tail = &rest[start + 2..];
        let Some(end) = tail.find(')') else {
            break;
        };
        out.push(tail[..end].to_owned());
        rest = &tail[end + 1..];
    }
    out
}

/// Returns the workspace-relative docs one index page links, as sorted paths.
fn indexed_docs(page: &str, page_rel: &str) -> Vec<String> {
    let base = Path::new(page_rel).parent().unwrap_or(Path::new(""));
    let mut out = Vec::new();
    for link in markdown_links(page) {
        let target = link.split('#').next().unwrap_or_default();
        if target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        let trimmed = target.trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        let joined = normalize(&base.join(trimmed));
        let resolved = if target.ends_with('/') {
            joined.join("README.md")
        } else if joined.extension().is_some_and(|ext| ext == "md") {
            joined
        } else {
            continue;
        };
        out.push(resolved.to_string_lossy().replace('\\', "/"));
    }
    sorted(out)
}

fn files_under(dir: &Path, keep: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current)
            .unwrap_or_else(|error| panic!("read_dir {}: {error}", current.display()));
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !keep(&name) {
                continue;
            }
            let rel = path
                .strip_prefix(runfiles_root())
                .expect("path under the workspace");
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    sorted(out)
}

/// Returns the workspace directory that owns one README path.
fn workspace_of(readme: &str) -> String {
    Path::new(readme)
        .parent()
        .expect("a README path has a parent")
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn every_published_user_doc_ships_in_the_workspace() {
    for doc in published() {
        assert!(
            runfiles_root().join(&doc).is_file(),
            "docs/site/BUILD.bazel publishes {doc}, which the tree does not ship"
        );
    }
}

#[test]
fn every_indexed_user_doc_is_published() {
    let published = published();
    for page_rel in INDEXES {
        for doc in indexed_docs(&read_runfiles(page_rel), page_rel) {
            assert!(
                published.contains(&doc),
                "{page_rel} links {doc}, which docs/site/BUILD.bazel does not publish"
            );
        }
    }
}

#[test]
fn every_command_page_is_published() {
    let published = published();
    for page in files_under(&runfiles_root().join("docs/cli/commands"), &|name| {
        name.ends_with(".md")
    }) {
        assert!(
            published.contains(&page),
            "{page} is not in the docs/site/BUILD.bazel USER_PROSE list"
        );
    }
}

#[test]
fn every_command_page_is_indexed() {
    let index = "docs/cli/commands/README.md";
    let mut indexed = indexed_docs(&read_runfiles(index), index);
    indexed.retain(|page| page.starts_with("docs/cli/commands/"));
    let mut pages = files_under(&runfiles_root().join("docs/cli/commands"), &|name| {
        name.ends_with(".md")
    });
    pages.retain(|page| page != index);
    assert_eq!(
        unique(indexed),
        unique(pages),
        "docs/cli/commands/README.md must link every page in docs/cli/commands/, so no page \
         ships unlinked"
    );
}

#[test]
fn the_examples_index_names_every_example_workspace_once() {
    let index = read_runfiles("examples/README.md");
    let named: Vec<String> = sorted(
        indexed_docs(&index, "examples/README.md")
            .iter()
            .map(|readme| workspace_of(readme))
            .collect(),
    );
    assert!(
        !named.is_empty(),
        "examples/README.md names no example workspace"
    );
    let workspaces: Vec<String> = files_under(&runfiles_root().join("examples"), &|name| {
        name == "README.md"
    })
    .iter()
    .filter(|readme| *readme != "examples/README.md")
    .map(|readme| workspace_of(readme))
    .collect();
    assert_eq!(
        named, workspaces,
        "examples/README.md must name every workspace that ships a README.md exactly once, and \
         nothing else"
    );
}
