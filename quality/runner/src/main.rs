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
use quality_runner::request::{load, Invocation};
use quality_runner::{
    real::{RealBackend, RealTool},
    run_pipeline, FileInput, StageSpec,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    #[error("{message}")]
    Args { message: String },
    #[error(transparent)]
    Request(#[from] quality_runner::request::RequestError),
    #[error("--request cannot be combined with other quality_runner flags")]
    MixedRequest,
    #[error("malformed --stage {spec:?}, want TOOL;classes;paths")]
    BadStage { spec: String },
    #[error("malformed --tool-binary {spec:?}, want TOOL=ABS_PATH")]
    BadToolBinary { spec: String },
    #[error("malformed --tool-config {spec:?}, want TOOL=MIRROR_REL")]
    BadToolConfig { spec: String },
    #[error("malformed --tool-edition {spec:?}, want TOOL=EDITION")]
    BadToolEdition { spec: String },
    #[error("malformed --tool-file {spec:?}, want TOOL=MIRROR_REL=EXEC")]
    BadToolFile { spec: String },
    #[error("malformed --tool-env {spec:?}, want TOOL=KEY=VALUE")]
    BadToolEnv { spec: String },
    #[error("malformed --upstream-diagnostics {spec:?}, want TOOL=EXEC_PATH")]
    BadUpstreamDiagnostics { spec: String },
    #[error("--{flag} is required")]
    MissingRequired { flag: &'static str },
    #[error("malformed --source {mapping:?}, want WS_PATH=EXEC")]
    BadSource { mapping: String },
    #[error("malformed --sibling {mapping:?}, want WS_PATH=EXEC")]
    BadSibling { mapping: String },
    #[error("malformed --resolve {mapping:?}, want WS_PATH=EXEC")]
    BadResolve { mapping: String },
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
    #[error("duplicate --tool-binary for {tool:?}")]
    DuplicateToolBinary { tool: String },
    #[error("--tool-config for unknown tool {tool:?}: pass --tool-binary first")]
    UnknownToolConfig { tool: String },
    #[error("duplicate --tool-config for {tool:?}")]
    DuplicateToolConfig { tool: String },
    #[error("--tool-edition for unknown tool {tool:?}: pass --tool-binary first")]
    UnknownToolEdition { tool: String },
    #[error("duplicate --tool-edition for {tool:?}")]
    DuplicateToolEdition { tool: String },
    #[error("cannot read tool file {rel:?} for {tool:?}: {detail}")]
    UnreadableToolFile {
        rel: String,
        tool: String,
        detail: String,
    },
    #[error("--tool-file for unknown tool {tool:?}: pass --tool-binary first")]
    UnknownToolFile { tool: String },
    #[error("--tool-env for unknown tool {tool:?}: pass --tool-binary first")]
    UnknownToolEnv { tool: String },
}

fn parse_args(args: &[String]) -> Result<Cli, RunnerError> {
    Cli::try_parse_from(std::iter::once("quality_runner").chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| RunnerError::Args {
            message: dx_output::parse_error(&error, args),
        })
}

fn parse_stage(spec: &str) -> Result<StageSpec, RunnerError> {
    let (tool_id, rest) = spec.split_once(';').ok_or_else(|| RunnerError::BadStage {
        spec: spec.to_owned(),
    })?;
    let (classes, paths) = rest.split_once(';').ok_or_else(|| RunnerError::BadStage {
        spec: spec.to_owned(),
    })?;
    Ok(StageSpec {
        tool_id: tool_id.to_owned(),
        class_ids: classes.split(',').map(str::to_owned).collect(),
        source_paths: paths.split(',').map(str::to_owned).collect(),
    })
}

fn parse_tool_binary(spec: &str) -> Result<(String, PathBuf), RunnerError> {
    let (tool, path) = spec
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolBinary {
            spec: spec.to_owned(),
        })?;
    if tool.is_empty() || path.is_empty() {
        return Err(RunnerError::BadToolBinary {
            spec: spec.to_owned(),
        });
    }
    Ok((tool.to_owned(), PathBuf::from(path)))
}

fn parse_tool_config(spec: &str) -> Result<(String, String), RunnerError> {
    let (tool, rel) = spec
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolConfig {
            spec: spec.to_owned(),
        })?;
    if tool.is_empty() || rel.is_empty() {
        return Err(RunnerError::BadToolConfig {
            spec: spec.to_owned(),
        });
    }
    Ok((tool.to_owned(), rel.to_owned()))
}

fn parse_tool_edition(spec: &str) -> Result<(String, String), RunnerError> {
    let (tool, edition) = spec
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolEdition {
            spec: spec.to_owned(),
        })?;
    if tool.is_empty() || edition.is_empty() {
        return Err(RunnerError::BadToolEdition {
            spec: spec.to_owned(),
        });
    }
    Ok((tool.to_owned(), edition.to_owned()))
}

fn parse_tool_file(spec: &str) -> Result<(String, String, String), RunnerError> {
    let (tool, rest) = spec
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolFile {
            spec: spec.to_owned(),
        })?;
    let (rel, exec) = rest
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolFile {
            spec: spec.to_owned(),
        })?;
    if tool.is_empty() || rel.is_empty() || exec.is_empty() {
        return Err(RunnerError::BadToolFile {
            spec: spec.to_owned(),
        });
    }
    Ok((tool.to_owned(), rel.to_owned(), exec.to_owned()))
}

fn parse_tool_env(spec: &str) -> Result<(String, String, String), RunnerError> {
    let (tool, rest) = spec
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolEnv {
            spec: spec.to_owned(),
        })?;
    let (key, value) = rest
        .split_once('=')
        .ok_or_else(|| RunnerError::BadToolEnv {
            spec: spec.to_owned(),
        })?;
    if tool.is_empty() || key.is_empty() {
        return Err(RunnerError::BadToolEnv {
            spec: spec.to_owned(),
        });
    }
    Ok((tool.to_owned(), key.to_owned(), value.to_owned()))
}

fn parse_upstream_diagnostics(spec: &str) -> Result<(String, PathBuf), RunnerError> {
    let (tool, path) = spec
        .split_once('=')
        .ok_or_else(|| RunnerError::BadUpstreamDiagnostics {
            spec: spec.to_owned(),
        })?;
    if tool.is_empty() || path.is_empty() {
        return Err(RunnerError::BadUpstreamDiagnostics {
            spec: spec.to_owned(),
        });
    }
    Ok((tool.to_owned(), PathBuf::from(path)))
}

fn parse_pair(
    spec: &str,
    malformed: fn(String) -> RunnerError,
) -> Result<(String, String), RunnerError> {
    let (left, right) = spec
        .split_once('=')
        .ok_or_else(|| malformed(spec.to_owned()))?;
    if left.is_empty() || right.is_empty() {
        return Err(malformed(spec.to_owned()));
    }
    Ok((left.to_owned(), right.to_owned()))
}

fn bad_source(mapping: String) -> RunnerError {
    RunnerError::BadSource { mapping }
}

fn bad_sibling(mapping: String) -> RunnerError {
    RunnerError::BadSibling { mapping }
}

fn bad_resolve(mapping: String) -> RunnerError {
    RunnerError::BadResolve { mapping }
}

fn unreadable_source(workspace: String, detail: String) -> RunnerError {
    RunnerError::UnreadableSource { workspace, detail }
}

fn unreadable_sibling(workspace: String, detail: String) -> RunnerError {
    RunnerError::UnreadableSibling { workspace, detail }
}

fn unreadable_resolve(workspace: String, detail: String) -> RunnerError {
    RunnerError::UnreadableResolve { workspace, detail }
}

fn parse_pairs(
    specs: &[String],
    malformed: fn(String) -> RunnerError,
) -> Result<Vec<(String, String)>, RunnerError> {
    specs
        .iter()
        .map(|spec| parse_pair(spec, malformed))
        .collect()
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

fn empty_tool(binary: PathBuf) -> RealTool {
    RealTool {
        binary,
        extra_env: Vec::new(),
        config_rel: None,
        edition: None,
        tool_files: Vec::new(),
        upstream_diagnostics: Vec::new(),
    }
}

fn insert_binary(
    tools: &mut BTreeMap<String, RealTool>,
    tool_id: String,
    binary: PathBuf,
    cwd: Option<PathBuf>,
) -> Result<(), RunnerError> {
    if tools.contains_key(&tool_id) {
        return Err(RunnerError::DuplicateToolBinary { tool: tool_id });
    }
    tools.insert(tool_id, empty_tool(absolute(cwd, binary)));
    Ok(())
}

fn add_upstream(
    tools: &mut BTreeMap<String, RealTool>,
    tool_id: String,
    exec: PathBuf,
    cwd: Option<PathBuf>,
) {
    tools
        .entry(tool_id)
        .or_insert_with(|| empty_tool(PathBuf::new()))
        .upstream_diagnostics
        .push(absolute(cwd, exec));
}

fn set_config(
    tools: &mut BTreeMap<String, RealTool>,
    tool_id: String,
    rel: String,
) -> Result<(), RunnerError> {
    let tool = tools
        .get_mut(&tool_id)
        .ok_or_else(|| RunnerError::UnknownToolConfig {
            tool: tool_id.clone(),
        })?;
    if tool.config_rel.is_some() {
        return Err(RunnerError::DuplicateToolConfig { tool: tool_id });
    }
    tool.config_rel = Some(rel);
    Ok(())
}

fn set_edition(
    tools: &mut BTreeMap<String, RealTool>,
    tool_id: String,
    edition: String,
) -> Result<(), RunnerError> {
    let tool = tools
        .get_mut(&tool_id)
        .ok_or_else(|| RunnerError::UnknownToolEdition {
            tool: tool_id.clone(),
        })?;
    if tool.edition.is_some() {
        return Err(RunnerError::DuplicateToolEdition { tool: tool_id });
    }
    tool.edition = Some(edition);
    Ok(())
}

fn add_tool_file(
    tools: &mut BTreeMap<String, RealTool>,
    tool_id: String,
    rel: String,
    bytes: Vec<u8>,
) -> Result<(), RunnerError> {
    let tool = tools
        .get_mut(&tool_id)
        .ok_or_else(|| RunnerError::UnknownToolFile {
            tool: tool_id.clone(),
        })?;
    tool.tool_files.push((rel, bytes));
    Ok(())
}

fn add_env(
    tools: &mut BTreeMap<String, RealTool>,
    tool_id: String,
    key: String,
    value: String,
) -> Result<(), RunnerError> {
    let tool = tools
        .get_mut(&tool_id)
        .ok_or_else(|| RunnerError::UnknownToolEnv {
            tool: tool_id.clone(),
        })?;
    tool.extra_env.push((key, value));
    Ok(())
}

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "producer")]
    producer: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "capability")]
    capability: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "output")]
    output: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    stage: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    source: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    sibling: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    resolve: Vec<String>,
    #[arg(long)]
    real: bool,
    #[arg(long, allow_hyphen_values = true, overrides_with = "scratch_parent")]
    scratch_parent: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    tool_binary: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    tool_config: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    tool_file: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    tool_edition: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    tool_env: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    upstream_diagnostics: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    request: Option<String>,
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
    let producer = cli
        .producer
        .ok_or(RunnerError::MissingRequired { flag: "producer" })?;
    let capability = cli
        .capability
        .ok_or(RunnerError::MissingRequired { flag: "capability" })?;
    let output = cli
        .output
        .ok_or(RunnerError::MissingRequired { flag: "output" })?;
    let mut stages: Vec<StageSpec> = Vec::with_capacity(cli.stage.len());
    for spec in &cli.stage {
        stages.push(parse_stage(spec)?);
    }
    let sources = parse_pairs(&cli.source, bad_source)?;
    let siblings = parse_pairs(&cli.sibling, bad_sibling)?;
    let resolves = parse_pairs(&cli.resolve, bad_resolve)?;
    let real = cli.real;
    let scratch_parent = cli.scratch_parent;
    let mut binaries: Vec<(String, PathBuf)> = Vec::with_capacity(cli.tool_binary.len());
    for spec in &cli.tool_binary {
        binaries.push(parse_tool_binary(spec)?);
    }
    let mut configs: Vec<(String, String)> = Vec::with_capacity(cli.tool_config.len());
    for spec in &cli.tool_config {
        configs.push(parse_tool_config(spec)?);
    }
    let mut editions: Vec<(String, String)> = Vec::with_capacity(cli.tool_edition.len());
    for spec in &cli.tool_edition {
        editions.push(parse_tool_edition(spec)?);
    }
    let mut tool_files: Vec<(String, String, String)> = Vec::with_capacity(cli.tool_file.len());
    for spec in &cli.tool_file {
        tool_files.push(parse_tool_file(spec)?);
    }
    let mut tool_env: Vec<(String, String, String)> = Vec::with_capacity(cli.tool_env.len());
    for spec in &cli.tool_env {
        tool_env.push(parse_tool_env(spec)?);
    }
    let mut upstream: Vec<(String, PathBuf)> = Vec::with_capacity(cli.upstream_diagnostics.len());
    for spec in &cli.upstream_diagnostics {
        upstream.push(parse_upstream_diagnostics(spec)?);
    }
    let cwd = std::env::current_dir().ok();
    let files = read_inputs(&sources, unreadable_source)?;
    let sibling_files = read_inputs(&siblings, unreadable_sibling)?;
    let resolve_files = read_inputs(&resolves, unreadable_resolve)?;
    if !real {
        let result = run_pipeline(&producer, &capability, &stages, &files).map_err(|e| {
            RunnerError::PipelineFailed {
                detail: e.to_string(),
            }
        })?;
        let bytes = encode_validated(&result).map_err(|e| RunnerError::InvalidResult {
            detail: e.to_string(),
        })?;
        dx_atomic_fs::write_atomic(std::path::Path::new(&output), &bytes).map_err(|e| {
            RunnerError::UnwritableOutput {
                output: output.clone(),
                detail: e.to_string(),
            }
        })?;
        return Ok(());
    }
    let mut tools: BTreeMap<String, RealTool> = BTreeMap::new();
    for (tool_id, binary) in binaries {
        insert_binary(&mut tools, tool_id, binary, cwd.clone())?;
    }
    for (tool_id, exec) in upstream {
        add_upstream(&mut tools, tool_id, exec, cwd.clone());
    }
    for (tool_id, rel) in configs {
        set_config(&mut tools, tool_id, rel)?;
    }
    for (tool_id, edition) in editions {
        set_edition(&mut tools, tool_id, edition)?;
    }
    for (tool_id, rel, exec) in tool_files {
        let bytes = std::fs::read(&exec).map_err(|e| RunnerError::UnreadableToolFile {
            rel: rel.clone(),
            tool: tool_id.clone(),
            detail: e.to_string(),
        })?;
        add_tool_file(&mut tools, tool_id, rel, bytes)?;
    }
    for (tool_id, key, value) in tool_env {
        add_env(&mut tools, tool_id, key, value)?;
    }
    let scratch_parent = scratch_parent
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let backend = RealBackend::new(tools, scratch_parent);
    let result = quality_runner::real::run_real_pipeline_with_resolve(
        &producer,
        &capability,
        &stages,
        &files,
        &sibling_files,
        &resolve_files,
        &backend,
    )
    .map_err(|e| RunnerError::PipelineFailed {
        detail: e.to_string(),
    })?;
    let bytes = encode_validated(&result).map_err(|e| RunnerError::InvalidResult {
        detail: e.to_string(),
    })?;
    dx_atomic_fs::write_atomic(std::path::Path::new(&output), &bytes).map_err(|e| {
        RunnerError::UnwritableOutput {
            output: output.clone(),
            detail: e.to_string(),
        }
    })?;
    Ok(())
}
// LCOV_EXCL_STOP - reason: end thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn tools_with(binary: &str) -> BTreeMap<String, RealTool> {
        let mut tools = BTreeMap::new();
        insert_binary(
            &mut tools,
            "ruff".to_owned(),
            PathBuf::from(binary),
            Some(PathBuf::from("/work")),
        )
        .expect("first binary wins");
        tools
    }

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    fn mapping_error(mapping: &str, malformed: fn(String) -> RunnerError) -> RunnerError {
        parse_pair(mapping, malformed).expect_err("mapping must be rejected")
    }

    #[test]
    fn stage_specs_split_tool_classes_and_paths() {
        let stage = parse_stage("ruff;fmt,lint;a.py,b.py").expect("stage");
        assert_eq!(stage.tool_id, "ruff");
        assert_eq!(stage.class_ids, ["fmt", "lint"]);
        assert_eq!(stage.source_paths, ["a.py", "b.py"]);
        assert_eq!(
            parse_stage("ruff").expect_err("no classes"),
            RunnerError::BadStage {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_stage("ruff;fmt").expect_err("no paths"),
            RunnerError::BadStage {
                spec: "ruff;fmt".to_owned()
            }
        );
    }

    #[test]
    fn tool_specs_reject_missing_and_empty_halves() {
        assert_eq!(
            parse_tool_binary("ruff").expect_err("no separator"),
            RunnerError::BadToolBinary {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_tool_binary("=bin/ruff").expect_err("no tool"),
            RunnerError::BadToolBinary {
                spec: "=bin/ruff".to_owned()
            }
        );
        assert_eq!(
            parse_tool_binary("ruff=").expect_err("no binary"),
            RunnerError::BadToolBinary {
                spec: "ruff=".to_owned()
            }
        );
        assert_eq!(
            parse_tool_binary("ruff=bin/ruff").expect("binary"),
            ("ruff".to_owned(), PathBuf::from("bin/ruff"))
        );
        assert_eq!(
            parse_tool_config("ruff").expect_err("no separator"),
            RunnerError::BadToolConfig {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_tool_config("ruff=").expect_err("no config"),
            RunnerError::BadToolConfig {
                spec: "ruff=".to_owned()
            }
        );
        assert_eq!(
            parse_tool_config("ruff=ruff.toml").expect("config"),
            ("ruff".to_owned(), "ruff.toml".to_owned())
        );
        assert_eq!(
            parse_tool_edition("ruff").expect_err("no separator"),
            RunnerError::BadToolEdition {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_tool_edition("=2021").expect_err("no tool"),
            RunnerError::BadToolEdition {
                spec: "=2021".to_owned()
            }
        );
        assert_eq!(
            parse_tool_edition("ruff=2021").expect("edition"),
            ("ruff".to_owned(), "2021".to_owned())
        );
        assert_eq!(
            parse_tool_file("ruff").expect_err("no separator"),
            RunnerError::BadToolFile {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_tool_file("ruff=toml").expect_err("no exec"),
            RunnerError::BadToolFile {
                spec: "ruff=toml".to_owned()
            }
        );
        assert_eq!(
            parse_tool_file("=toml=bin/toml").expect_err("no tool"),
            RunnerError::BadToolFile {
                spec: "=toml=bin/toml".to_owned()
            }
        );
        assert_eq!(
            parse_tool_file("ruff==bin/toml").expect_err("no config"),
            RunnerError::BadToolFile {
                spec: "ruff==bin/toml".to_owned()
            }
        );
        assert_eq!(
            parse_tool_file("ruff=toml=bin/toml").expect("tool file"),
            ("ruff".to_owned(), "toml".to_owned(), "bin/toml".to_owned())
        );
        assert_eq!(
            parse_tool_env("ruff").expect_err("no separator"),
            RunnerError::BadToolEnv {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_tool_env("=KEY=1").expect_err("no tool"),
            RunnerError::BadToolEnv {
                spec: "=KEY=1".to_owned()
            }
        );
        assert_eq!(
            parse_tool_env("ruff=KEY").expect_err("no value separator"),
            RunnerError::BadToolEnv {
                spec: "ruff=KEY".to_owned()
            }
        );
        assert_eq!(
            parse_tool_env("ruff==1").expect_err("no key"),
            RunnerError::BadToolEnv {
                spec: "ruff==1".to_owned()
            }
        );
        assert_eq!(
            parse_tool_env("ruff=KEY=1").expect("tool env"),
            ("ruff".to_owned(), "KEY".to_owned(), "1".to_owned())
        );
        assert_eq!(
            parse_upstream_diagnostics("ruff").expect_err("no separator"),
            RunnerError::BadUpstreamDiagnostics {
                spec: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_upstream_diagnostics("ruff=").expect_err("no exec path"),
            RunnerError::BadUpstreamDiagnostics {
                spec: "ruff=".to_owned()
            }
        );
        assert_eq!(
            parse_upstream_diagnostics("ruff=out/diag").expect("upstream"),
            ("ruff".to_owned(), PathBuf::from("out/diag"))
        );
    }

    #[test]
    fn source_sibling_and_resolve_mappings_reject_empty_halves() {
        for mapping in ["ws", "=exec", "ws="] {
            assert_eq!(
                mapping_error(mapping, bad_source),
                RunnerError::BadSource {
                    mapping: mapping.to_owned()
                }
            );
            assert_eq!(
                mapping_error(mapping, bad_sibling),
                RunnerError::BadSibling {
                    mapping: mapping.to_owned()
                }
            );
            assert_eq!(
                mapping_error(mapping, bad_resolve),
                RunnerError::BadResolve {
                    mapping: mapping.to_owned()
                }
            );
        }
        assert_eq!(
            parse_pairs(&owned(&["ws=exec"]), bad_source).expect("source"),
            [("ws".to_owned(), "exec".to_owned())]
        );
        assert_eq!(parse_pairs(&[], bad_source).expect("none"), []);
    }

    #[test]
    fn argv_errors_carry_a_message_and_valid_flags_parse() {
        let cli = parse_args(&owned(&[
            "--producer",
            "//pkg:one",
            "--real",
            "--stage",
            "ruff;fmt;a.py",
        ]))
        .expect("valid flags");
        assert_eq!(cli.producer.as_deref(), Some("//pkg:one"));
        assert!(cli.real);
        assert_eq!(cli.stage, ["ruff;fmt;a.py"]);
        let error = match parse_args(&owned(&["--nope"])) {
            Ok(_) => panic!("unknown flag must fail"),
            Err(error) => error,
        };
        match error {
            RunnerError::Args { message } => assert!(!message.is_empty(), "{message}"),
            other => panic!("unexpected error: {other:?}"),
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
    fn binaries_register_once_each() {
        let mut tools = tools_with("bin/ruff");
        assert_eq!(tools["ruff"].binary, PathBuf::from("/work/bin/ruff"));
        assert!(tools["ruff"].extra_env.is_empty());
        assert!(tools["ruff"].config_rel.is_none());
        assert!(tools["ruff"].edition.is_none());
        assert!(tools["ruff"].tool_files.is_empty());
        assert!(tools["ruff"].upstream_diagnostics.is_empty());
        assert_eq!(
            insert_binary(
                &mut tools,
                "ruff".to_owned(),
                PathBuf::from("bin/ruff"),
                Some(PathBuf::from("/work")),
            )
            .expect_err("duplicate tool"),
            RunnerError::DuplicateToolBinary {
                tool: "ruff".to_owned()
            }
        );
    }

    #[test]
    fn upstream_diagnostics_attach_to_known_and_unknown_tools() {
        let mut tools = tools_with("bin/ruff");
        add_upstream(
            &mut tools,
            "ruff".to_owned(),
            PathBuf::from("out/ruff.diag"),
            Some(PathBuf::from("/work")),
        );
        assert_eq!(
            tools["ruff"].upstream_diagnostics,
            [PathBuf::from("/work/out/ruff.diag")]
        );
        add_upstream(
            &mut tools,
            "ty".to_owned(),
            PathBuf::from("/abs/ty.diag"),
            None,
        );
        assert_eq!(
            tools["ty"].upstream_diagnostics,
            [PathBuf::from("/abs/ty.diag")]
        );
        assert_eq!(tools["ty"].binary, PathBuf::new());
        add_upstream(
            &mut tools,
            "ty".to_owned(),
            PathBuf::from("/abs/ty2.diag"),
            None,
        );
        assert_eq!(
            tools["ty"].upstream_diagnostics,
            [
                PathBuf::from("/abs/ty.diag"),
                PathBuf::from("/abs/ty2.diag")
            ]
        );
    }

    #[test]
    fn config_and_edition_apply_once_to_a_declared_tool() {
        let mut tools = tools_with("bin/ruff");
        assert_eq!(
            set_config(&mut tools, "ty".to_owned(), "ty.toml".to_owned())
                .expect_err("unknown tool"),
            RunnerError::UnknownToolConfig {
                tool: "ty".to_owned()
            }
        );
        set_config(&mut tools, "ruff".to_owned(), "ruff.toml".to_owned()).expect("config");
        assert_eq!(tools["ruff"].config_rel.as_deref(), Some("ruff.toml"));
        assert_eq!(
            set_config(&mut tools, "ruff".to_owned(), "other.toml".to_owned())
                .expect_err("duplicate config"),
            RunnerError::DuplicateToolConfig {
                tool: "ruff".to_owned()
            }
        );
        assert_eq!(
            set_edition(&mut tools, "ty".to_owned(), "2021".to_owned()).expect_err("unknown tool"),
            RunnerError::UnknownToolEdition {
                tool: "ty".to_owned()
            }
        );
        set_edition(&mut tools, "ruff".to_owned(), "2021".to_owned()).expect("edition");
        assert_eq!(tools["ruff"].edition.as_deref(), Some("2021"));
        assert_eq!(
            set_edition(&mut tools, "ruff".to_owned(), "2024".to_owned())
                .expect_err("duplicate edition"),
            RunnerError::DuplicateToolEdition {
                tool: "ruff".to_owned()
            }
        );
    }

    #[test]
    fn tool_files_and_env_need_a_declared_binary() {
        let mut tools = tools_with("bin/ruff");
        assert_eq!(
            add_tool_file(
                &mut tools,
                "ty".to_owned(),
                "ty.toml".to_owned(),
                b"x".to_vec(),
            )
            .expect_err("unknown tool"),
            RunnerError::UnknownToolFile {
                tool: "ty".to_owned()
            }
        );
        add_tool_file(
            &mut tools,
            "ruff".to_owned(),
            "ruff.toml".to_owned(),
            b"x".to_vec(),
        )
        .expect("tool file");
        assert_eq!(
            tools["ruff"].tool_files,
            [("ruff.toml".to_owned(), b"x".to_vec())]
        );
        assert_eq!(
            add_env(
                &mut tools,
                "ty".to_owned(),
                "KEY".to_owned(),
                "1".to_owned()
            )
            .expect_err("unknown tool"),
            RunnerError::UnknownToolEnv {
                tool: "ty".to_owned()
            }
        );
        add_env(
            &mut tools,
            "ruff".to_owned(),
            "KEY".to_owned(),
            "1".to_owned(),
        )
        .expect("tool env");
        assert_eq!(
            tools["ruff"].extra_env,
            [("KEY".to_owned(), "1".to_owned())]
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
