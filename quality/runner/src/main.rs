#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{error::ErrorKind, Parser};
use quality_result::encode_validated;
use quality_runner::{
    real::{RealBackend, RealTool},
    run_pipeline, FileInput, StageSpec,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    #[error("{message}")]
    Args { message: String },
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

fn main() {
    dx_output::init_diagnostics(false);
    if let Err(error) = run() {
        tracing::error!("quality_runner: {error}");
        std::process::exit(1);
    }
}

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
}

fn invalid_token(error: &clap::Error) -> String {
    dx_output::invalid_token(error)
}

fn parse_error(error: clap::Error, args: &[String]) -> String {
    let token = invalid_token(&error);
    match error.kind() {
        ErrorKind::UnknownArgument => {
            let echoed = dx_output::recover_unknown_token(args, &token);
            format!("unknown flag {echoed:?}")
        }
        ErrorKind::InvalidValue => {
            let flag = dx_output::leading_flag(&token);
            format!("missing value for {flag}")
        }
        _ => dx_output::first_line(&error),
    }
}

fn parse_args(args: &[String]) -> Result<Cli, RunnerError> {
    Cli::try_parse_from(std::iter::once("quality_runner").chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| RunnerError::Args {
            message: parse_error(error, args),
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
    let mut sources: Vec<(String, String)> = Vec::with_capacity(cli.source.len());
    for mapping in &cli.source {
        let (workspace, exec) = mapping
            .split_once('=')
            .ok_or_else(|| RunnerError::BadSource {
                mapping: mapping.clone(),
            })?;
        sources.push((workspace.to_owned(), exec.to_owned()));
    }
    let mut siblings: Vec<(String, String)> = Vec::with_capacity(cli.sibling.len());
    for mapping in &cli.sibling {
        let (workspace, exec) = mapping
            .split_once('=')
            .ok_or_else(|| RunnerError::BadSibling {
                mapping: mapping.clone(),
            })?;
        siblings.push((workspace.to_owned(), exec.to_owned()));
    }
    let mut resolves: Vec<(String, String)> = Vec::with_capacity(cli.resolve.len());
    for mapping in &cli.resolve {
        let (workspace, exec) = mapping
            .split_once('=')
            .ok_or_else(|| RunnerError::BadResolve {
                mapping: mapping.clone(),
            })?;
        resolves.push((workspace.to_owned(), exec.to_owned()));
    }
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
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let mut files = Vec::with_capacity(sources.len());
    for (workspace, exec) in &sources {
        let bytes = std::fs::read(exec).map_err(|e| RunnerError::UnreadableSource {
            workspace: workspace.clone(),
            detail: e.to_string(),
        })?;
        files.push(FileInput {
            path: workspace.clone(),
            bytes,
        });
    }
    let mut sibling_files = Vec::with_capacity(siblings.len());
    for (workspace, exec) in &siblings {
        let bytes = std::fs::read(exec).map_err(|e| RunnerError::UnreadableSibling {
            workspace: workspace.clone(),
            detail: e.to_string(),
        })?;
        sibling_files.push(FileInput {
            path: workspace.clone(),
            bytes,
        });
    }
    let mut resolve_files = Vec::with_capacity(resolves.len());
    for (workspace, exec) in &resolves {
        let bytes = std::fs::read(exec).map_err(|e| RunnerError::UnreadableResolve {
            workspace: workspace.clone(),
            detail: e.to_string(),
        })?;
        resolve_files.push(FileInput {
            path: workspace.clone(),
            bytes,
        });
    }
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
        if tools.contains_key(&tool_id) {
            return Err(RunnerError::DuplicateToolBinary { tool: tool_id });
        }
        let absolute = if binary.is_absolute() {
            binary
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&binary))
                .unwrap_or(binary)
        };
        tools.insert(
            tool_id,
            RealTool {
                binary: absolute,
                extra_env: Vec::new(),
                config_rel: None,
                edition: None,
                tool_files: Vec::new(),
                upstream_diagnostics: Vec::new(),
            },
        );
    }
    for (tool_id, exec) in upstream {
        let absolute = if exec.is_absolute() {
            exec
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&exec))
                .unwrap_or(exec)
        };
        tools
            .entry(tool_id.clone())
            .or_insert(RealTool {
                binary: PathBuf::new(),
                extra_env: Vec::new(),
                config_rel: None,
                edition: None,
                tool_files: Vec::new(),
                upstream_diagnostics: Vec::new(),
            })
            .upstream_diagnostics
            .push(absolute);
    }
    for (tool_id, rel) in configs {
        let tool = tools
            .get_mut(&tool_id)
            .ok_or_else(|| RunnerError::UnknownToolConfig {
                tool: tool_id.clone(),
            })?;
        if tool.config_rel.is_some() {
            return Err(RunnerError::DuplicateToolConfig { tool: tool_id });
        }
        tool.config_rel = Some(rel);
    }
    for (tool_id, edition) in editions {
        let tool = tools
            .get_mut(&tool_id)
            .ok_or_else(|| RunnerError::UnknownToolEdition {
                tool: tool_id.clone(),
            })?;
        if tool.edition.is_some() {
            return Err(RunnerError::DuplicateToolEdition { tool: tool_id });
        }
        tool.edition = Some(edition);
    }
    for (tool_id, rel, exec) in tool_files {
        let bytes = std::fs::read(&exec).map_err(|e| RunnerError::UnreadableToolFile {
            rel: rel.clone(),
            tool: tool_id.clone(),
            detail: e.to_string(),
        })?;
        let tool = tools
            .get_mut(&tool_id)
            .ok_or_else(|| RunnerError::UnknownToolFile {
                tool: tool_id.clone(),
            })?;
        tool.tool_files.push((rel, bytes));
    }
    for (tool_id, key, value) in tool_env {
        let tool = tools
            .get_mut(&tool_id)
            .ok_or_else(|| RunnerError::UnknownToolEnv {
                tool: tool_id.clone(),
            })?;
        tool.extra_env.push((key, value));
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
