use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use dx_testing::{read_runfiles, runfiles_root};

const INDEXES: [&str; 3] = [
    "docs/README.md",
    "docs/cli/commands/README.md",
    "examples/README.md",
];

const NOT_PUBLISHED: [&str; 3] = ["AGENTS.md", "CONTRIBUTING.md", "README.md"];

const COMMAND_INDEX: &str = "docs/cli/commands/README.md";

const SURFACE_HEADING: &str = "## Supported Surface";

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

/// Returns the curated book's source labels as workspace-relative paths.
fn published() -> Vec<String> {
    let book = read_runfiles("tools/docs/site/user/book.bzl");
    let mut names = Vec::new();
    let mut pages = 0;
    let mut call = String::new();
    let mut inside = false;
    for line in book.lines() {
        let line = line.trim();
        if line.starts_with("mdbook_page(") {
            pages += 1;
            inside = true;
            call.clear();
        }
        if inside {
            call.push_str(line);
            call.push(' ');
        }
        if !inside || !line.ends_with("),") {
            continue;
        }
        inside = false;
        let Some(label) = call
            .split('"')
            .find(|part| part.starts_with("//"))
            .map(|label| label.to_owned())
        else {
            panic!("a curated book page names no source label: {}", call.trim());
        };
        let rest = label.trim_start_matches('/');
        let (package, file) = rest
            .split_once(':')
            .unwrap_or_else(|| panic!("curated book label {label} names no file"));
        let mut path = PathBuf::new();
        if !package.is_empty() {
            path.push(package);
        }
        path.push(file);
        names.push(path.to_string_lossy().replace('\\', "/"));
    }
    assert!(pages > 0, "tools/docs/site/user/book.bzl curates no pages");
    assert_eq!(
        pages,
        names.len(),
        "every mdbook_page in tools/docs/site/user/book.bzl must name one source label"
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

/// Returns the sorted, unique non-example `@rules_dx//<pkg>:<file>.bzl` labels in one text.
fn bzl_labels(text: &str) -> Vec<String> {
    const PREFIX: &str = "@rules_dx//";
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(PREFIX) {
        let tail = &rest[start..];
        let end = tail[PREFIX.len()..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || "_./:-".contains(c)))
            .map_or(tail.len(), |end| end + PREFIX.len());
        let label = tail[..end].trim_end_matches('.');
        rest = &tail[end..];
        let Some((package, file)) = label[PREFIX.len()..].split_once(':') else {
            continue;
        };
        if file.ends_with(".bzl") && !package.starts_with("examples/") {
            out.push(label.to_owned());
        }
    }
    unique(out)
}

/// Returns the Supported Surface section of the command index and the page without it.
fn split_surface(page: &str) -> (&str, String) {
    let start = page
        .find(SURFACE_HEADING)
        .unwrap_or_else(|| panic!("{COMMAND_INDEX} has no {SURFACE_HEADING} section"));
    let end = page[start + SURFACE_HEADING.len()..]
        .find("\n## ")
        .map_or(page.len(), |end| start + SURFACE_HEADING.len() + end + 1);
    (
        &page[start..end],
        format!("{}{}", &page[..start], &page[end..]),
    )
}

/// Returns the `.bzl` labels the Supported Surface section lists.
fn supported_surface() -> Vec<String> {
    bzl_labels(split_surface(&read_runfiles(COMMAND_INDEX)).0)
}

/// Returns each `.bzl` label user docs and examples load, with the files that load it.
fn documented_loads() -> BTreeMap<String, Vec<String>> {
    let scanned: Vec<String> = files_under(&runfiles_root(), &|name| {
        name.ends_with(".md")
            || name.ends_with(".bzl")
            || name == "BUILD.bazel"
            || name.ends_with(".BUILD.bazel")
    })
    .into_iter()
    .filter(|rel| rel.ends_with(".md") || rel.starts_with("examples/"))
    .collect();
    assert!(
        scanned
            .iter()
            .any(|rel| rel.starts_with("examples/") && rel.ends_with("/BUILD.bazel")),
        "user_docs_test sees no examples/ BUILD.bazel; //examples:example_starlark is missing \
         from its data"
    );
    let mut loads: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in scanned {
        let text = read_runfiles(&rel);
        let text = if rel == COMMAND_INDEX {
            split_surface(&text).1
        } else {
            text
        };
        for label in bzl_labels(&text) {
            loads.entry(label).or_default().push(rel.clone());
        }
    }
    loads
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
            "the curated book publishes {doc}, which the tree does not ship"
        );
    }
}

#[test]
fn the_curated_book_publishes_no_agent_or_repository_page() {
    let published = published();
    for page in NOT_PUBLISHED {
        assert!(
            !published.iter().any(|doc| doc == page),
            "the curated book publishes {page}, which is not a user doc"
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
                "{page_rel} links {doc}, which the curated book does not publish"
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
            "{page} is not in the curated book in tools/docs/site/user/book.bzl"
        );
    }
}

#[test]
fn every_command_page_is_indexed() {
    let commands = "docs/cli/commands/README.md";
    let mut indexed = indexed_docs(&read_runfiles(commands), commands);
    indexed.retain(|page| page.starts_with("docs/cli/commands/"));
    let mut pages = files_under(&runfiles_root().join("docs/cli/commands"), &|name| {
        name.ends_with(".md")
    });
    pages.retain(|page| page != commands);
    assert_eq!(
        unique(indexed),
        unique(pages),
        "docs/cli/commands/README.md must link every page in docs/cli/commands/, so no page \
         ships unlinked"
    );

    let top = "docs/README.md";
    let mut indexed = indexed_docs(&read_runfiles(top), top);
    indexed.retain(|page| page.starts_with("docs/cli/commands/"));
    let pages = files_under(&runfiles_root().join("docs/cli/commands"), &|name| {
        name.ends_with(".md")
    });
    assert_eq!(
        unique(indexed),
        unique(pages),
        "docs/README.md must link every page in docs/cli/commands/"
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

#[test]
fn readme_docs_site_link_targets_the_devhusdk_host() {
    let readme = read_runfiles("README.md");
    let sites: Vec<String> = markdown_links(&readme)
        .into_iter()
        .filter(|link| link.contains(".github.io"))
        .collect();
    assert_eq!(
        sites,
        ["https://devhusdk.github.io/rules_dx/"],
        "README.md must link the docs site on the devhusdk Pages host exactly once"
    );
}

#[test]
fn the_supported_surface_lists_every_documented_load() {
    let surface = supported_surface();
    let missing: Vec<String> = documented_loads()
        .into_iter()
        .filter(|(label, _)| !surface.contains(label))
        .map(|(label, files)| format!("{label} (loaded by {})", files.join(", ")))
        .collect();
    assert!(
        missing.is_empty(),
        "{COMMAND_INDEX} {SURFACE_HEADING} must list every .bzl file user docs and examples \
         load; missing:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_supported_surface_load_is_documented() {
    let loads = documented_loads();
    let unused: Vec<String> = supported_surface()
        .into_iter()
        .filter(|label| !loads.contains_key(label))
        .collect();
    assert!(
        unused.is_empty(),
        "{COMMAND_INDEX} {SURFACE_HEADING} lists .bzl files no user doc or example loads:\n{}",
        unused.join("\n")
    );
}

#[test]
fn bzl_labels_reads_loads_extensions_and_prose() {
    let sample = r#"load("@rules_dx//a/rules:defs.bzl", "a_library")
sdk = use_extension("@rules_dx//a/sdk:repos.bzl", "a_sdk")
Load `@rules_dx//b:prose.bzl`, or see @rules_dx//c:end.bzl.
Run `@rules_dx//:dx` and skip (@rules_dx//examples/x:y.bzl).
"#;
    assert_eq!(
        bzl_labels(sample),
        [
            "@rules_dx//a/rules:defs.bzl",
            "@rules_dx//a/sdk:repos.bzl",
            "@rules_dx//b:prose.bzl",
            "@rules_dx//c:end.bzl",
        ]
    );
}

#[test]
fn the_surface_split_keeps_the_rest_of_the_page() {
    let page = "# Top\n\n## Supported Surface\n\n- `@rules_dx//a:b.bzl`\n\n## Next\n\nTail.\n";
    let (section, rest) = split_surface(page);
    assert_eq!(
        section,
        "## Supported Surface\n\n- `@rules_dx//a:b.bzl`\n\n"
    );
    assert_eq!(rest, "# Top\n\n## Next\n\nTail.\n");
}
