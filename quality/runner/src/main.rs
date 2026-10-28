#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::Parser;
use quality_result::encode_validated;
use quality_runner::{
    real::{RealBackend, RealTool},
    request::RequestTool,
    FileInput,
};

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
    dir: &std::path::Path,
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

/// Builds the backend tool for one validated request entry.
fn real_tool(
    tool_id: &str,
    entry: &RequestTool,
    cwd: Option<PathBuf>,
) -> Result<RealTool, RunnerError> {
    let binary = entry
        .binary
        .clone()
        .map(|binary| absolute(cwd.clone(), binary))
        .unwrap_or_default();
    let mut tool_files = Vec::with_capacity(entry.files.len());
    for (rel, exec) in &entry.files {
        let unreadable = |error: std::io::Error| RunnerError::UnreadableToolFile {
            rel: rel.clone(),
            tool: tool_id.to_owned(),
            detail: error.to_string(),
        };
        if std::path::Path::new(exec).is_dir() {
            read_tool_directory(rel, std::path::Path::new(exec), &mut tool_files)
                .map_err(unreadable)?;
        } else {
            tool_files.push((rel.clone(), std::fs::read(exec).map_err(unreadable)?));
        }
    }
    Ok(RealTool {
        binary,
        extra_env: entry.env.clone(),
        config_rel: entry.config_rel.clone(),
        edition: entry.edition.clone(),
        dialect: entry.dialect.clone(),
        tool_files,
        upstream_diagnostics: entry
            .upstream
            .iter()
            .map(|exec| absolute(cwd.clone(), exec.clone()))
            .collect(),
        path_tools: entry
            .path_tools
            .iter()
            .map(|(name, binary)| (name.clone(), absolute(cwd.clone(), binary.clone())))
            .collect(),
        search_paths: entry
            .search_paths
            .iter()
            .map(|dir| site_packages(absolute(cwd.clone(), dir.clone())))
            .collect(),
    })
}

fn run_request(request_path: &str, output: &str) -> Result<(), RunnerError> {
    let bytes = std::fs::read(request_path).map_err(|error| RunnerError::UnreadableRequest {
        path: request_path.to_owned(),
        detail: error.to_string(),
    })?;
    let request = quality_runner::request::parse_real_request(&bytes).map_err(|error| {
        RunnerError::BadRequest {
            detail: error.to_string(),
        }
    })?;
    let cwd = std::env::current_dir().ok();
    let files = read_inputs(&request.sources, unreadable_source)?;
    let sibling_files = read_inputs(&request.siblings, unreadable_sibling)?;
    let resolve_files = read_inputs(&request.resolves, unreadable_resolve)?;
    let mut tools: BTreeMap<String, RealTool> = BTreeMap::new();
    for (tool_id, entry) in &request.tools {
        tools.insert(tool_id.clone(), real_tool(tool_id, entry, cwd.clone())?);
    }
    let scratch_parent = request
        .scratch_parent
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let backend = RealBackend::new(tools, scratch_parent);
    let result = quality_runner::real::run_real_pipeline_with_resolve(
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
    dx_atomic_fs::write_atomic(std::path::Path::new(output), &bytes).map_err(|error| {
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

fn absolute(cwd: Option<PathBuf>, path: PathBuf) -> PathBuf {
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
}

fn main() {
    dx_output::init_diagnostics(false);
    if let Err(error) = run() {
        tracing::error!("quality_runner: {error}");
        std::process::exit(1);
    }
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

// LCOV_EXCL_START - reason: thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run() -> Result<(), RunnerError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse_args(&args)?;
    run_request(&cli.request, &cli.output)
}
// LCOV_EXCL_STOP - reason: end thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn cli_takes_only_request_and_output() {
        let cli =
            parse_args(&owned(&["--request", "r.json", "--output", "o.pb"])).expect("valid flags");
        assert_eq!(cli.request, "r.json");
        assert_eq!(cli.output, "o.pb");
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
            let error = match parse_args(&owned(&argv)) {
                Ok(_) => panic!("{argv:?} must fail"),
                Err(error) => error,
            };
            match error {
                RunnerError::Args { message } => assert!(!message.is_empty(), "{message}"),
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn request_failures_name_the_request() {
        let missing = std::env::temp_dir().join("quality_runner_absent_request.json");
        match run_request(
            &missing.display().to_string(),
            &missing.display().to_string(),
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
            other => panic!("unexpected error: {other:?}"),
        }
        let dir = tempfile::tempdir().expect("scratch");
        let malformed = dir.path().join("malformed.json");
        std::fs::write(&malformed, b"{not json").expect("request file");
        match run_request(
            &malformed.display().to_string(),
            &dir.path().join("out.pb").display().to_string(),
        )
        .expect_err("malformed request")
        {
            RunnerError::BadRequest { detail } => assert!(!detail.is_empty(), "{detail}"),
            other => panic!("unexpected error: {other:?}"),
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
        std::fs::create_dir(&empty).expect("empty");
        let entry = RequestTool {
            binary: None,
            config_rel: None,
            edition: None,
            dialect: None,
            files: vec![
                ("owned/0/ebin".to_owned(), tree.display().to_string()),
                ("owned/1/empty".to_owned(), empty.display().to_string()),
            ],
            env: Vec::new(),
            upstream: Vec::new(),
            search_paths: Vec::new(),
            path_tools: Vec::new(),
        };
        let tool = real_tool("dialyzer", &entry, None).expect("tool");
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
            files: vec![("ruff.toml".to_owned(), file.display().to_string())],
            env: vec![
                ("B".to_owned(), "2".to_owned()),
                ("A".to_owned(), "1".to_owned()),
            ],
            upstream: vec![PathBuf::from("out/ruff.diag")],
            search_paths: vec![venv.clone()],
            path_tools: Vec::new(),
        };
        let tool = real_tool("ruff", &entry, cwd.clone()).expect("tool");
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
            files: Vec::new(),
            env: Vec::new(),
            upstream: vec![PathBuf::from("/abs/ty.diag")],
            search_paths: Vec::new(),
            path_tools: Vec::new(),
        };
        let tool = real_tool("ty", &upstream_only, cwd.clone()).expect("upstream only");
        assert_eq!(tool.binary, PathBuf::new());
        assert_eq!(tool.upstream_diagnostics, [PathBuf::from("/abs/ty.diag")]);
        let missing = RequestTool {
            files: vec![(
                "ty.toml".to_owned(),
                dir.path().join("absent.toml").display().to_string(),
            )],
            ..upstream_only
        };
        match real_tool("ty", &missing, cwd) {
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
            absolute(Some(PathBuf::from("/work")), PathBuf::from("bin/ruff")),
            PathBuf::from("/work/bin/ruff")
        );
        assert_eq!(
            absolute(None, PathBuf::from("bin/ruff")),
            PathBuf::from("bin/ruff")
        );
        assert_eq!(
            absolute(Some(PathBuf::from("/work")), PathBuf::from("/usr/bin/ruff")),
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
        let missing = std::env::temp_dir().join("quality_runner_absent_input.py");
        let mappings = [("a.py".to_owned(), missing.display().to_string())];
        let cases: [(fn(String, String) -> RunnerError, &str); 3] = [
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
                other => panic!("unexpected error: {other:?}"),
            }
            assert!(error.to_string().starts_with(expected), "{error}");
        }
    }
}
