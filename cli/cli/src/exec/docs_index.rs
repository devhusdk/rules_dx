//! Builds one browsable index from the Rust API shards a `dx docs` build produced.

use super::common::*;
use dx_plan::plan::{DOCS_SHARD_OUTPUT_GROUP, DOCS_TOOLS_OUTPUT_GROUP};
use dx_process::Runner;
use std::path::{Path, PathBuf};

pub(crate) const CODE_INDEX_FAILED: &str = "docs_index_failed";

const INDEX_DIR: &str = ".dx/local/docs";

const BOOK_TOML: &str = "[book]\ntitle = \"API reference\"\nlanguage = \"en\"\nsrc = \"src\"\n";

const SUMMARY: &str = "# Summary\n\n- [API reference](README.md)\n";

const README: &str = "# API reference\n\nThe public API of the Rust libraries in scope.\n";

struct Tools {
    aggregate: PathBuf,
    mdbook: PathBuf,
    site_build: PathBuf,
}

fn failed(detail: String) -> (String, String) {
    (CODE_INDEX_FAILED.to_owned(), detail)
}

/// The shard files the build reported, sorted and without repeats.
pub(crate) fn built_shards(bep: &Path, workspace: &Path) -> Result<Vec<PathBuf>, (String, String)> {
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
#[allow(clippy::disallowed_methods)] // info takes no build settings
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

fn built_tools(
    bep: &Path,
    workspace: &Path,
    execution_root: Option<&Path>,
) -> Result<Tools, (String, String)> {
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
    let mut take = |name: &str| {
        tools
            .remove(name)
            .ok_or_else(|| failed(format!("the docs build did not report the {name} tool")))
    };
    Ok(Tools {
        aggregate: take("aggregate")?,
        mdbook: take("mdbook")?,
        site_build: take("site_build")?,
    })
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

fn aggregate_argv(tool: &Path, shards: &[PathBuf], work: &Path) -> Vec<String> {
    let mut argv = vec![text(tool)];
    for shard in shards {
        argv.push("--shard".to_owned());
        argv.push(text(shard));
    }
    for (flag, name) in [
        ("--summary", "api_summary.md"),
        ("--api", "api.json"),
        ("--api-tree", "api"),
        ("--records", "records.json"),
    ] {
        argv.push(flag.to_owned());
        argv.push(text(&work.join(name)));
    }
    argv
}

fn site_build_argv(tools: &Tools, work: &Path, out: &Path) -> Vec<String> {
    let mut argv = vec![text(&tools.site_build)];
    for (flag, value) in [
        ("--mdbook", text(&tools.mdbook)),
        ("--book", text(&work.join("book.toml"))),
        ("--summary", text(&work.join("SUMMARY.md"))),
        ("--manifest", text(&work.join("pages.tsv"))),
        ("--out", text(out)),
        ("--api-tree", text(&work.join("api"))),
        ("--api-summary", text(&work.join("api_summary.md"))),
    ] {
        argv.push(flag.to_owned());
        argv.push(value);
    }
    argv
}

fn run_tool(
    runner: &dyn Runner,
    workspace: &Path,
    argv: &[String],
) -> Result<(), (String, String)> {
    let name = argv.first().map_or("", String::as_str);
    match runner.run(argv, workspace, &[]) {
        Ok(status) if status.code == Some(0) => Ok(()),
        Ok(status) => Err(failed(match status.code {
            Some(code) => format!("{name} exited with code {code}"),
            None => format!("{name} was terminated by a signal"),
        })),
        Err(error) => Err(failed(format!("cannot run {name}: {error}"))),
    }
}

/// Renders the shards into `<workspace>/.dx/local/docs/index` and returns that tree.
pub(crate) fn build_index(
    bep: &Path,
    workspace: &Path,
    execution_root: Option<&Path>,
    shards: &[PathBuf],
    runner: &dyn Runner,
) -> Result<PathBuf, (String, String)> {
    let tools = built_tools(bep, workspace, execution_root)?;
    let root = workspace.join(INDEX_DIR);
    let work = root.join("work");
    let out = root.join("index");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&out);
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
        std::fs::write(&path, content)
            .map_err(|error| failed(format!("cannot write {}: {error}", path.display())))?;
    }
    run_tool(
        runner,
        workspace,
        &aggregate_argv(&tools.aggregate, shards, &work),
    )?;
    run_tool(runner, workspace, &site_build_argv(&tools, &work, &out))?;
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

    #[test]
    fn aggregate_gets_every_shard_once_in_order() {
        let shards = [PathBuf::from("/w/a/a.shard"), PathBuf::from("/w/b/b.shard")];
        let argv = aggregate_argv(
            Path::new("/t/agg"),
            &shards,
            Path::new("/w/.dx/local/docs/work"),
        );
        let argv: Vec<String> = argv.iter().map(|arg| arg.replace('\\', "/")).collect();
        assert_eq!(argv[0], "/t/agg");
        assert_eq!(
            argv[1..5],
            ["--shard", "/w/a/a.shard", "--shard", "/w/b/b.shard"]
        );
        assert!(
            argv.contains(&"/w/.dx/local/docs/work/api".to_owned()),
            "{argv:?}"
        );
    }

    #[test]
    fn site_build_reads_the_generated_book_and_api() {
        let tools = Tools {
            aggregate: PathBuf::from("/t/agg"),
            mdbook: PathBuf::from("/t/mdbook"),
            site_build: PathBuf::from("/t/site_build"),
        };
        let argv = site_build_argv(&tools, Path::new("/w/work"), Path::new("/w/index"));
        let joined = argv.join(" ").replace('\\', "/");
        assert!(
            joined.starts_with("/t/site_build --mdbook /t/mdbook"),
            "{joined}"
        );
        assert!(joined.contains("--book /w/work/book.toml"), "{joined}");
        assert!(joined.contains("--out /w/index"), "{joined}");
        assert!(
            joined.contains("--api-summary /w/work/api_summary.md"),
            "{joined}"
        );
    }
}
