use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{
    FileInput,
    real::{RealBackend, RealTool},
    request::RequestTool,
};
use clap::Parser;
use quality_result::encode_validated;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    #[error("{message}")]
    Args { message: String },
    #[error("cannot read {workspace:?}: {detail}")]
    UnreadableSource { workspace: String, detail: String },
    #[error("cannot read sibling {workspace:?}: {detail}")]
    UnreadableSibling { workspace: String, detail: String },
    #[error("cannot read resolve {workspace:?}: {detail}")]
    UnreadableResolve { workspace: String, detail: String },
    #[error("pipeline failed: {detail}")]
    PipelineFailed { detail: String },
    #[error("invalid result: {detail}")]
    InvalidResult { detail: String },
    #[error("cannot write {output:?}: {detail}")]
    UnwritableOutput { output: String, detail: String },
    #[error("cannot read tool file {rel:?} for {tool:?}: {detail}")]
    UnreadableToolFile {
        rel: String,
        tool: String,
        detail: String,
    },
    #[error("cannot read request {path:?}: {detail}")]
    UnreadableRequest { path: String, detail: String },
    #[error("invalid request: {detail}")]
    BadRequest { detail: String },
}

fn parse_args(args: &[String]) -> Result<Cli, RunnerError> {
    Cli::try_parse_from(std::iter::once("quality_runner").chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| RunnerError::Args {
            message: dx_output::parse_error(&error, args),
        })
}

fn unreadable_source(workspace: String, detail: String) -> RunnerError {
    RunnerError::UnreadableSource { workspace, detail }
}

/// Stages every file under a tool directory (a tree artifact) as `rel/<path inside>`, in sorted
/// order. An empty directory stages nothing.
fn read_tool_directory(
    rel: &str,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> std::io::Result<()> {
    let mut children: Vec<std::fs::DirEntry> =
        std::fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let child_rel = format!("{rel}/{}", child.file_name().to_string_lossy());
        if child.path().is_dir() {
            read_tool_directory(&child_rel, &child.path(), out)?;
        } else {
            out.push((child_rel, std::fs::read(child.path())?));
        }
    }
    Ok(())
}

/// Reads one tool file, or every file under a tool directory, into `out` at `rel`.
fn stage_tool_file(
    tool_id: &str,
    rel: &str,
    exec: &str,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), RunnerError> {
    let unreadable = |error: std::io::Error| RunnerError::UnreadableToolFile {
        rel: rel.to_owned(),
        tool: tool_id.to_owned(),
        detail: error.to_string(),
    };
    if Path::new(exec).is_dir() {
        read_tool_directory(rel, Path::new(exec), out).map_err(unreadable)
    } else {
        out.push((rel.to_owned(), std::fs::read(exec).map_err(unreadable)?));
        Ok(())
    }
}

/// Builds the backend tool for one validated request entry. File-list entries whose rel is a
/// checked source or already staged are skipped.
fn real_tool(
    tool_id: &str,
    entry: &RequestTool,
    cwd: Option<&Path>,
    sources: &BTreeSet<&str>,
) -> Result<RealTool, RunnerError> {
    let binary = entry
        .binary
        .clone()
        .map(|binary| absolute(cwd, binary))
        .unwrap_or_default();
    let mut tool_files = Vec::with_capacity(entry.files.len());
    let mut staged: BTreeSet<String> = BTreeSet::new();
    for (rel, exec) in &entry.files {
        stage_tool_file(tool_id, rel, exec, &mut tool_files)?;
        staged.insert(rel.clone());
    }
    for list in &entry.file_lists {
        let unreadable = |detail: String| RunnerError::UnreadableToolFile {
            rel: list.display().to_string(),
            tool: tool_id.to_owned(),
            detail,
        };
        let text = std::fs::read_to_string(list).map_err(|error| unreadable(error.to_string()))?;
        for line in text.lines().filter(|line| !line.is_empty()) {
            let (rel, exec) = line
                .split_once('\t')
                .ok_or_else(|| unreadable(format!("line {line:?} is not rel<TAB>exec")))?;
            if sources.contains(rel) || !staged.insert(rel.to_owned()) {
                continue;
            }
            stage_tool_file(tool_id, rel, exec, &mut tool_files)?;
        }
    }
    Ok(RealTool {
        binary,
        extra_env: entry.env.clone(),
        config_rel: entry.config_rel.clone(),
        edition: entry.edition.clone(),
        dialect: entry.dialect.clone(),
        args: entry.args.clone(),
        tool_files,
        upstream_diagnostics: entry
            .upstream
            .iter()
            .map(|exec| absolute(cwd, exec.clone()))
            .collect(),
        path_tools: entry
            .path_tools
            .iter()
            .map(|(name, binary)| (name.clone(), absolute(cwd, binary.clone())))
            .collect(),
        search_paths: entry
            .search_paths
            .iter()
            .map(|dir| site_packages(absolute(cwd, dir.clone())))
            .collect(),
    })
}

/// Returns the result of the call.
///
/// # Errors
///
/// Returns the error describing the first step that failed.
pub fn run_request(
    request_path: &str,
    output: &str,
    shard: Option<&str>,
) -> Result<(), RunnerError> {
    let bytes = std::fs::read(request_path).map_err(|error| RunnerError::UnreadableRequest {
        path: request_path.to_owned(),
        detail: error.to_string(),
    })?;
    let request =
        crate::request::parse_real_request(&bytes).map_err(|error| RunnerError::BadRequest {
            detail: error.to_string(),
        })?;
    let cwd = std::env::current_dir().ok();
    let files = read_inputs(&request.sources, unreadable_source)?;
    let sibling_files = read_inputs(&request.siblings, unreadable_sibling)?;
    let resolve_files = read_inputs(&request.resolves, unreadable_resolve)?;
    let mut tools: BTreeMap<String, RealTool> = BTreeMap::new();
    let source_rels: BTreeSet<&str> = request
        .sources
        .iter()
        .map(|(workspace, _)| workspace.as_str())
        .collect();
    for (tool_id, entry) in &request.tools {
        tools.insert(
            tool_id.clone(),
            real_tool(tool_id, entry, cwd.as_deref(), &source_rels)?,
        );
    }
    let scratch_parent = request
        .scratch_parent
        .map_or_else(std::env::temp_dir, PathBuf::from);
    let backend = RealBackend::new(tools, scratch_parent)
        .with_shard(shard.unwrap_or_default())
        .with_unsafe_fixes(request.unsafe_fixes);
    let result = crate::real::run_real_pipeline_with_resolve(
        &request.producer,
        &request.capability,
        &request.stages,
        &files,
        &sibling_files,
        &resolve_files,
        &backend,
    )
    .map_err(|error| RunnerError::PipelineFailed {
        detail: error.to_string(),
    })?;
    let bytes = encode_validated(&result).map_err(|error| RunnerError::InvalidResult {
        detail: error.to_string(),
    })?;
    dx_atomic_fs::write_scratch(Path::new(output), &bytes).map_err(|error| {
        RunnerError::UnwritableOutput {
            output: output.to_owned(),
            detail: error.to_string(),
        }
    })?;
    Ok(())
}

fn unreadable_sibling(workspace: String, detail: String) -> RunnerError {
    RunnerError::UnreadableSibling { workspace, detail }
}

fn unreadable_resolve(workspace: String, detail: String) -> RunnerError {
    RunnerError::UnreadableResolve { workspace, detail }
}

fn read_inputs(
    mappings: &[(String, String)],
    unreadable: fn(String, String) -> RunnerError,
) -> Result<Vec<FileInput>, RunnerError> {
    let mut inputs = Vec::with_capacity(mappings.len());
    for (workspace, exec) in mappings {
        let bytes = std::fs::read(exec)
            .map_err(|error| unreadable(workspace.clone(), error.to_string()))?;
        inputs.push(FileInput {
            path: workspace.clone(),
            bytes,
        });
    }
    Ok(inputs)
}

fn absolute(cwd: Option<&Path>, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        match cwd {
            Some(base) => base.join(path),
            None => path,
        }
    }
}

fn site_packages(dir: PathBuf) -> PathBuf {
    let lib = dir.join("lib");
    if let Ok(entries) = std::fs::read_dir(&lib) {
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("site-packages"))
            .filter(|path| path.is_dir())
            .collect();
        found.sort();
        if let Some(first) = found.into_iter().next() {
            return first;
        }
    }
    dir
}

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "request")]
    request: String,
    #[arg(long, allow_hyphen_values = true, overrides_with = "output")]
    output: String,
    #[arg(long, allow_hyphen_values = true, overrides_with = "shard")]
    shard: Option<String>,
}

/// Runs `quality_runner` with its arguments (without the program name) and returns the exit code.
pub fn run_cli(args: &[String]) -> i32 {
    dx_output::init_diagnostics(false);
    match parse_args(args)
        .and_then(|cli| run_request(&cli.request, &cli.output, cli.shard.as_deref()))
    {
        Ok(()) => 0,
        Err(error) => {
            tracing::error!("quality_runner: {error}");
            1
        }
    }
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn cli_takes_request_output_and_shard() {
        let cli =
            parse_args(&owned(&["--request", "r.json", "--output", "o.pb"])).expect("valid flags");
        assert_eq!(cli.request, "r.json");
        assert_eq!(cli.output, "o.pb");
        assert_eq!(cli.shard, None);
        let cli = parse_args(&owned(&[
            "--request",
            "r.json",
            "--output",
            "o.pb",
            "--shard",
            "py",
        ]))
        .expect("valid flags");
        assert_eq!(cli.shard.as_deref(), Some("py"));
        for argv in [
            vec!["--producer", "x"],
            vec!["--capability", "lint"],
            vec!["--stage", "a;b;c"],
            vec!["--source", "a=b"],
            vec!["--real"],
            vec!["--nope"],
            vec!["--request", "r.json"],
            vec!["--output", "o.pb"],
        ] {
            let Err(error) = parse_args(&owned(&argv)) else {
                panic!("{argv:?} must fail")
            };
            match error {
                RunnerError::Args { message } => assert!(!message.is_empty(), "{message}"),
                other @ (RunnerError::UnreadableSource { .. }
                | RunnerError::UnreadableSibling { .. }
                | RunnerError::UnreadableResolve { .. }
                | RunnerError::PipelineFailed { .. }
                | RunnerError::InvalidResult { .. }
                | RunnerError::UnwritableOutput { .. }
                | RunnerError::UnreadableToolFile { .. }
                | RunnerError::UnreadableRequest { .. }
                | RunnerError::BadRequest { .. }) => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn request_failures_name_the_request() {
        let missing = std::env::temp_dir().join("quality_runner_absent_request.json");
        match run_request(
            &missing.display().to_string(),
            &missing.display().to_string(),
            None,
        )
        .expect_err("missing request")
        {
            RunnerError::UnreadableRequest { path, detail } => {
                assert!(
                    path.ends_with("quality_runner_absent_request.json"),
                    "{path}"
                );
                assert!(!detail.is_empty(), "{path}");
            }
            other @ (RunnerError::Args { .. }
            | RunnerError::UnreadableSource { .. }
            | RunnerError::UnreadableSibling { .. }
            | RunnerError::UnreadableResolve { .. }
            | RunnerError::PipelineFailed { .. }
            | RunnerError::InvalidResult { .. }
            | RunnerError::UnwritableOutput { .. }
            | RunnerError::UnreadableToolFile { .. }
            | RunnerError::BadRequest { .. }) => panic!("unexpected error: {other:?}"),
        }
        let dir = tempfile::tempdir().expect("scratch");
        let malformed = dir.path().join("malformed.json");
        std::fs::write(&malformed, b"{not json").expect("request file");
        match run_request(
            &malformed.display().to_string(),
            &dir.path().join("out.pb").display().to_string(),
            None,
        )
        .expect_err("malformed request")
        {
            RunnerError::BadRequest { detail } => assert!(!detail.is_empty(), "{detail}"),
            other @ (RunnerError::Args { .. }
            | RunnerError::UnreadableSource { .. }
            | RunnerError::UnreadableSibling { .. }
            | RunnerError::UnreadableResolve { .. }
            | RunnerError::PipelineFailed { .. }
            | RunnerError::InvalidResult { .. }
            | RunnerError::UnwritableOutput { .. }
            | RunnerError::UnreadableToolFile { .. }
            | RunnerError::UnreadableRequest { .. }) => {
                panic!("unexpected error: {other:?}")
            }
        }
    }

    #[test]
    fn directory_tool_files_are_staged_file_by_file_in_sorted_order() {
        let dir = tempfile::tempdir().expect("scratch");
        let tree = dir.path().join("ebin");
        std::fs::create_dir_all(tree.join("sub")).expect("layout");
        std::fs::write(tree.join("b.beam"), b"b").expect("file");
        std::fs::write(tree.join("a.app"), b"a").expect("file");
        std::fs::write(tree.join("sub/c.beam"), b"c").expect("file");
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).expect("empty");
        let entry = RequestTool {
            binary: None,
            config_rel: None,
            edition: None,
            dialect: None,
            args: Vec::new(),
            files: vec![
                ("owned/0/ebin".to_owned(), tree.display().to_string()),
                ("owned/1/empty".to_owned(), empty.display().to_string()),
            ],
            env: Vec::new(),
            upstream: Vec::new(),
            search_paths: Vec::new(),
            path_tools: Vec::new(),
            file_lists: Vec::new(),
        };
        let tool = real_tool("dialyzer", &entry, None, &BTreeSet::new()).expect("tool");
        assert_eq!(
            tool.tool_files,
            [
                ("owned/0/ebin/a.app".to_owned(), b"a".to_vec()),
                ("owned/0/ebin/b.beam".to_owned(), b"b".to_vec()),
                ("owned/0/ebin/sub/c.beam".to_owned(), b"c".to_vec()),
            ]
        );
    }

    #[test]
    fn file_lists_stage_each_rel_once_and_skip_sources() {
        let dir = tempfile::tempdir().expect("scratch");
        let types = dir.path().join("dep.d.ts");
        std::fs::write(&types, b"export {};").expect("file");
        let config = dir.path().join("tsconfig.json");
        std::fs::write(&config, b"{}").expect("file");
        let list = dir.path().join("eslint-staged.txt");
        std::fs::write(
            &list,
            format!(
                "pkg/a.ts\t{config}\npkg/dep.d.ts\t{types}\npkg/tsconfig.json\t{config}\npkg/dep.d.ts\t{config}\n",
                config = config.display(),
                types = types.display(),
            ),
        )
        .expect("list");
        let entry = RequestTool {
            binary: None,
            config_rel: None,
            edition: None,
            dialect: None,
            args: Vec::new(),
            files: vec![("pkg/tsconfig.json".to_owned(), types.display().to_string())],
            env: Vec::new(),
            upstream: vec![PathBuf::from("out/eslint.diag")],
            search_paths: Vec::new(),
            path_tools: Vec::new(),
            file_lists: vec![list],
        };
        let tool = real_tool("eslint", &entry, None, &BTreeSet::from(["pkg/a.ts"])).expect("tool");
        assert_eq!(
            tool.tool_files,
            [
                ("pkg/tsconfig.json".to_owned(), b"export {};".to_vec()),
                ("pkg/dep.d.ts".to_owned(), b"export {};".to_vec()),
            ]
        );
        let bad = dir.path().join("bad.txt");
        std::fs::write(&bad, b"no-tab\n").expect("list");
        let entry = RequestTool {
            file_lists: vec![bad],
            files: Vec::new(),
            ..entry
        };
        match real_tool("eslint", &entry, None, &BTreeSet::new()) {
            Err(RunnerError::UnreadableToolFile { detail, .. }) => {
                assert!(detail.contains("no-tab"), "{detail}");
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("a malformed file list must fail"),
        }
    }

    #[test]
    fn request_tools_become_real_tools() {
        let dir = tempfile::tempdir().expect("scratch");
        let file = dir.path().join("ruff.toml");
        std::fs::write(&file, b"line-length = 1\n").expect("tool file");
        let venv = dir.path().join("venv");
        let site = venv.join("lib/python3.12/site-packages");
        std::fs::create_dir_all(&site).expect("layout");
        let cwd = Some(PathBuf::from("/work"));
        let entry = RequestTool {
            binary: Some(PathBuf::from("bin/ruff")),
            config_rel: Some("ruff.toml".to_owned()),
            edition: Some("3.9".to_owned()),
            dialect: None,
            args: Vec::new(),
            files: vec![("ruff.toml".to_owned(), file.display().to_string())],
            env: vec![
                ("B".to_owned(), "2".to_owned()),
                ("A".to_owned(), "1".to_owned()),
            ],
            upstream: vec![PathBuf::from("out/ruff.diag")],
            search_paths: vec![venv.clone()],
            path_tools: Vec::new(),
            file_lists: Vec::new(),
        };
        let tool = real_tool("ruff", &entry, cwd.as_deref(), &BTreeSet::new()).expect("tool");
        assert_eq!(tool.binary, PathBuf::from("/work/bin/ruff"));
        assert_eq!(tool.config_rel.as_deref(), Some("ruff.toml"));
        assert_eq!(tool.edition.as_deref(), Some("3.9"));
        assert_eq!(
            tool.tool_files,
            [("ruff.toml".to_owned(), b"line-length = 1\n".to_vec())]
        );
        assert_eq!(
            tool.extra_env,
            [
                ("B".to_owned(), "2".to_owned()),
                ("A".to_owned(), "1".to_owned())
            ]
        );
        assert_eq!(
            tool.upstream_diagnostics,
            [PathBuf::from("/work/out/ruff.diag")]
        );
        assert_eq!(tool.search_paths, [site]);
        let upstream_only = RequestTool {
            binary: None,
            config_rel: None,
            edition: None,
            dialect: None,
            args: Vec::new(),
            files: Vec::new(),
            env: Vec::new(),
            upstream: vec![PathBuf::from("/abs/ty.diag")],
            search_paths: Vec::new(),
            path_tools: Vec::new(),
            file_lists: Vec::new(),
        };
        let tool = real_tool("ty", &upstream_only, cwd.as_deref(), &BTreeSet::new())
            .expect("upstream only");
        assert_eq!(tool.binary, PathBuf::new());
        assert_eq!(tool.upstream_diagnostics, [PathBuf::from("/abs/ty.diag")]);
        let missing = RequestTool {
            files: vec![(
                "ty.toml".to_owned(),
                dir.path().join("absent.toml").display().to_string(),
            )],
            ..upstream_only
        };
        match real_tool("ty", &missing, cwd.as_deref(), &BTreeSet::new()) {
            Err(RunnerError::UnreadableToolFile { rel, tool, detail }) => {
                assert_eq!(rel, "ty.toml");
                assert_eq!(tool, "ty");
                assert!(!detail.is_empty(), "{detail}");
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("an unreadable tool file must fail"),
        }
    }

    #[test]
    fn relative_paths_resolve_against_the_workspace_only_when_known() {
        assert_eq!(
            absolute(Some(Path::new("/work")), PathBuf::from("bin/ruff")),
            PathBuf::from("/work/bin/ruff")
        );
        assert_eq!(
            absolute(None, PathBuf::from("bin/ruff")),
            PathBuf::from("bin/ruff")
        );
        assert_eq!(
            absolute(Some(Path::new("/work")), PathBuf::from("/usr/bin/ruff")),
            PathBuf::from("/usr/bin/ruff")
        );
    }

    #[test]
    fn site_packages_prefers_the_wheel_install_layout() {
        let root = tempfile::tempdir().expect("tempdir");
        let site = root.path().join("lib/python3.12/site-packages");
        std::fs::create_dir_all(&site).expect("layout");
        assert_eq!(site_packages(root.path().to_path_buf()), site);
        let bare = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            site_packages(bare.path().to_path_buf()),
            bare.path().to_path_buf()
        );
    }

    #[test]
    fn inputs_are_read_from_the_exec_path_under_the_workspace_name() {
        let dir = tempfile::tempdir().expect("scratch");
        let exec = dir.path().join("a.py");
        std::fs::write(&exec, b"x = 1\n").expect("source file");
        let mappings = [("a.py".to_owned(), exec.display().to_string())];
        let inputs = read_inputs(&mappings, unreadable_source).expect("inputs");
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].path, "a.py");
        assert_eq!(inputs[0].bytes, b"x = 1\n");
        assert_eq!(
            read_inputs(&mappings, unreadable_sibling).expect("siblings")[0].path,
            "a.py"
        );
        assert_eq!(
            read_inputs(&mappings, unreadable_resolve).expect("resolves")[0].path,
            "a.py"
        );
        assert_eq!(read_inputs(&[], unreadable_source).expect("none"), []);
    }

    #[test]
    fn a_missing_input_names_the_role_that_could_not_read_it() {
        type Unreadable = fn(String, String) -> RunnerError;
        let missing = std::env::temp_dir().join("quality_runner_absent_input.py");
        let mappings = [("a.py".to_owned(), missing.display().to_string())];
        let cases: [(Unreadable, &str); 3] = [
            (unreadable_source, "cannot read \"a.py\""),
            (unreadable_sibling, "cannot read sibling \"a.py\""),
            (unreadable_resolve, "cannot read resolve \"a.py\""),
        ];
        for (unreadable, expected) in cases {
            let error = read_inputs(&mappings, unreadable).expect_err("missing input");
            match &error {
                RunnerError::UnreadableSource { workspace, detail }
                | RunnerError::UnreadableSibling { workspace, detail }
                | RunnerError::UnreadableResolve { workspace, detail } => {
                    assert_eq!(workspace, "a.py");
                    assert!(!detail.is_empty(), "{error:?}");
                }
                other @ (RunnerError::Args { .. }
                | RunnerError::PipelineFailed { .. }
                | RunnerError::InvalidResult { .. }
                | RunnerError::UnwritableOutput { .. }
                | RunnerError::UnreadableToolFile { .. }
                | RunnerError::UnreadableRequest { .. }
                | RunnerError::BadRequest { .. }) => panic!("unexpected error: {other:?}"),
            }
            assert!(error.to_string().starts_with(expected), "{error}");
        }
    }
}
