//! Builds one browsable index from the Rust API shards a `dx docs` build produced.

use dx_exec_core::common::collect_output_account;
use dx_plan::plan::{DOCS_SHARD_OUTPUT_GROUP, DOCS_TOOLS_OUTPUT_GROUP};
use std::path::{Path, PathBuf};

pub(crate) const CODE_INDEX_FAILED: &str = "docs_index_failed";

const INDEX_DIR: &str = ".dx/local/docs";

const BOOK_TOML: &str = "[book]\ntitle = \"API reference\"\nlanguage = \"en\"\nsrc = \"src\"\n";

const SUMMARY: &str = "# Summary\n\n- [API reference](README.md)\n";

const README: &str = "# API reference\n\nThe public API of the Rust libraries in scope.\n";

fn failed(detail: String) -> dx_error::DxError {
    dx_error::DxError::failure(detail)
}

/// The shard files the build reported, sorted and without repeats.
pub(crate) fn built_shards(
    bep: &Path,
    workspace: &Path,
) -> Result<Vec<PathBuf>, dx_error::DxError> {
    let account = collect_output_account(bep, DOCS_SHARD_OUTPUT_GROUP, workspace)?;
    let mut shards: Vec<PathBuf> = account
        .targets
        .into_iter()
        .flat_map(|target| target.artifacts)
        .map(|artifact| workspace.join(artifact.exec_path))
        .collect();
    shards.sort();
    shards.dedup();
    Ok(shards)
}

/// Resolves an exec path; files of other repos sit in the execution root, not the workspace.
fn tool_path(workspace: &Path, execution_root: Option<&Path>, path: &str) -> PathBuf {
    match execution_root {
        Some(root) if path.starts_with("external/") => {
            dx_tool_resolve::join_under(&root.to_string_lossy(), path)
        }
        _ => workspace.join(path),
    }
}

/// The execution root `bazel info` reports, or `None` when it cannot be read.
#[expect(clippy::disallowed_methods, reason = "info takes no build settings")]
pub(crate) fn execution_root(
    query_runner: &dyn dx_plan::resolve::QueryRunner,
    workspace: &Path,
    startup: &[String],
) -> Option<PathBuf> {
    let mut argv = dx_process::startup_argv(startup);
    argv.extend(["info".to_owned(), "execution_root".to_owned()]);
    let result = query_runner.run_info(&argv, workspace).ok()?;
    let text = String::from_utf8_lossy(&result.stdout);
    let line = text.trim();
    (result.code == Some(0) && !line.is_empty()).then(|| PathBuf::from(line))
}

/// The mdBook binary the docs build reported in its tools manifest.
fn built_mdbook(
    bep: &Path,
    workspace: &Path,
    execution_root: Option<&Path>,
) -> Result<PathBuf, dx_error::DxError> {
    let account = collect_output_account(bep, DOCS_TOOLS_OUTPUT_GROUP, workspace)?;
    let mut tools = std::collections::BTreeMap::new();
    for artifact in account
        .targets
        .into_iter()
        .flat_map(|target| target.artifacts)
    {
        for line in String::from_utf8_lossy(&artifact.bytes).lines() {
            if let Some((name, path)) = line.split_once('\t') {
                tools.insert(name.to_owned(), tool_path(workspace, execution_root, path));
            }
        }
    }
    tools
        .remove("mdbook")
        .ok_or_else(|| failed("the docs build did not report the mdbook tool".to_owned()))
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

fn aggregate_options(shards: &[PathBuf], work: &Path) -> dx_site_aggregate::Options {
    dx_site_aggregate::Options {
        shards: shards.iter().map(|shard| text(shard)).collect(),
        source_url: String::new(),
        summary: text(&work.join("api_summary.md")),
        api: text(&work.join("api.json")),
        api_tree: text(&work.join("api")),
        records: text(&work.join("records.json")),
    }
}

fn site_build_options(mdbook: &Path, work: &Path, out: &Path) -> dx_site_build::Options {
    dx_site_build::Options {
        mdbook: text(mdbook),
        book: text(&work.join("book.toml")),
        summary: text(&work.join("SUMMARY.md")),
        manifest: text(&work.join("pages.tsv")),
        out: text(out),
        api_tree: text(&work.join("api")),
        api_summary: text(&work.join("api_summary.md")),
    }
}

/// Turns a tool's refusal text, empty on success, into an error named after the tool.
fn refused(tool: &str, refusal: String) -> Result<(), dx_error::DxError> {
    if refusal.is_empty() {
        Ok(())
    } else {
        Err(failed(format!("{tool}: {refusal}")))
    }
}

/// Renders the shards into `<workspace>/.dx/local/docs/index` and returns that tree.
pub(crate) fn build_index(
    bep: &Path,
    workspace: &Path,
    execution_root: Option<&Path>,
    shards: &[PathBuf],
) -> Result<PathBuf, dx_error::DxError> {
    let mdbook = built_mdbook(bep, workspace, execution_root)?;
    let root = workspace.join(INDEX_DIR);
    let work = root.join("work");
    let out = root.join("index");
    dx_atomic_fs::discard_dir(&work);
    dx_atomic_fs::discard_dir(&out);
    std::fs::create_dir_all(&work)
        .map_err(|error| failed(format!("cannot create {}: {error}", work.display())))?;
    let page = work.join("README.md");
    let files = [
        (work.join("book.toml"), BOOK_TOML.to_owned()),
        (work.join("SUMMARY.md"), SUMMARY.to_owned()),
        (page.clone(), README.to_owned()),
        (
            work.join("pages.tsv"),
            format!("README.md\t{}\n", page.display()),
        ),
    ];
    for (path, content) in files {
        dx_atomic_fs::write_scratch(&path, content.as_bytes())
            .map_err(|error| failed(format!("cannot write {}: {error}", path.display())))?;
    }
    refused(
        "site_aggregate",
        dx_site_aggregate::run(&aggregate_options(shards, &work))
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default(),
    )?;
    refused(
        "site_build",
        dx_site_build::run(&site_build_options(&mdbook, &work, &out)),
    )?;
    if !out.join("index.html").is_file() {
        return Err(failed(format!(
            "the index has no index.html at {}",
            out.display()
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_tools_resolve_under_the_execution_root() {
        let root = Path::new("/out/execroot/_main");
        let path = tool_path(Path::new("/w"), Some(root), "external/mdbook+/bin/mdbook");
        assert_eq!(
            path,
            root.join("external")
                .join("mdbook+")
                .join("bin")
                .join("mdbook")
        );
        let path = tool_path(Path::new("/w"), Some(root), "bazel-out/k8/bin/agg");
        assert_eq!(path, Path::new("/w").join("bazel-out/k8/bin/agg"));
    }

    fn slashes(path: &str) -> String {
        path.replace('\\', "/")
    }

    #[test]
    fn aggregate_gets_every_shard_once_in_order() {
        let shards = [PathBuf::from("/w/a/a.shard"), PathBuf::from("/w/b/b.shard")];
        let options = aggregate_options(&shards, Path::new("/w/.dx/local/docs/work"));
        let shards: Vec<String> = options.shards.iter().map(|s| slashes(s)).collect();
        assert_eq!(shards, ["/w/a/a.shard", "/w/b/b.shard"]);
        assert_eq!(slashes(&options.api_tree), "/w/.dx/local/docs/work/api");
        assert!(options.source_url.is_empty(), "{options:?}");
    }

    #[test]
    fn site_build_reads_the_generated_book_and_api() {
        let options = site_build_options(
            Path::new("/t/mdbook"),
            Path::new("/w/work"),
            Path::new("/w/index"),
        );
        assert_eq!(slashes(&options.mdbook), "/t/mdbook");
        assert_eq!(slashes(&options.book), "/w/work/book.toml");
        assert_eq!(slashes(&options.out), "/w/index");
        assert_eq!(slashes(&options.api_summary), "/w/work/api_summary.md");
    }

    #[test]
    fn a_refusal_names_its_tool() {
        assert!(refused("site_build", String::new()).is_ok());
        let error = refused("site_build", "no pages".to_owned()).err();
        assert!(
            error.is_some_and(|error| error.to_string().contains("site_build: no pages")),
            "refusal lost its tool name"
        );
    }

    #[test]
    fn an_undecodable_shard_fails_the_index_in_process() {
        let dir = dx_test_scratch::scratch("docs-index-shard").unwrap();
        let shard = dir.path().join("bad.shard");
        std::fs::write(&shard, b"not a shard").unwrap();
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        let refusal = dx_site_aggregate::run(&aggregate_options(&[shard], &work))
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(refusal.contains("bad.shard"), "{refusal}");
    }
}
