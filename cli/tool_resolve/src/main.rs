//! Resolves one pinned tool label to the absolute artifact path CI exports.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use dx_tool_resolve::{
    append_env_file, context_dir, env_line, resolve, validate_var, CommandBazel,
};

#[derive(Debug, Parser)]
#[command(
    name = "tool_resolve",
    about = "Resolve one pinned tool label to the absolute artifact path CI exports",
    long_about = "tool_resolve runs `bazel cquery --output=files` for one pinned tool label, \
takes the last output line as the artifact path, joins it with `bazel info output_base`, \
refuses a missing file, and appends VAR=path to the GitHub environment file so later \
steps read the hermetic tool without host parsing.\n\n--label LABEL names the tool \
(e.g. @dx_tools//:gitleaks); --var NAME names the exported variable (e.g. \
DX_GITLEAKS_BIN); --bazel PATH selects the Bazel binary (default bazel); \
--workspace DIR runs the inner Bazel commands there (default \
BUILD_WORKSPACE_DIRECTORY, the `bazel run` invocation root, else the current \
directory); --env-file PATH overrides GITHUB_ENV; --print writes VAR=path to standard output \
instead of a file; trailing arguments after -- reach the inner cquery unchanged.\n\n\
Exit codes: 0 success, 2 usage error, 1 resolution failure."
)]
struct Cli {
    /// Pinned Bazel label of the tool to resolve.
    #[arg(long)]
    label: String,
    /// Environment variable name receiving the absolute path.
    #[arg(long)]
    var: String,
    /// Bazel binary the resolution runs through.
    #[arg(long, default_value = "bazel")]
    bazel: PathBuf,
    /// Workspace the inner Bazel commands run in.
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Environment file receiving VAR=path instead of GITHUB_ENV.
    #[arg(long)]
    env_file: Option<PathBuf>,
    /// Write VAR=path to standard output instead of a file.
    #[arg(long)]
    print: bool,
    /// Extra arguments reaching the inner cquery unchanged.
    #[arg(last = true)]
    bazel_args: Vec<String>,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    match run(cli) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<String, dx_tool_resolve::ResolveError> {
    validate_var(&cli.var)?;
    let env_file = if cli.print {
        None
    } else {
        Some(env_target(cli.env_file.as_deref())?)
    };
    let current =
        std::env::current_dir().map_err(|error| dx_tool_resolve::ResolveError::Usage {
            detail: format!("could not read the current directory: {error}"),
        })?;
    let dir = context_dir(
        cli.workspace.as_deref(),
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").as_deref(),
        &current,
    );
    let bazel = CommandBazel {
        bin: cli.bazel,
        dir,
    };
    let path = resolve(&bazel, &cli.label, &cli.bazel_args)?;
    let line = env_line(&cli.var, &path)?;
    if let Some(env_file) = env_file {
        append_env_file(&env_file, &line).map_err(|error| {
            dx_tool_resolve::ResolveError::EnvWrite {
                var: cli.var.clone(),
                path: env_file.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }
    Ok(line.trim_end().to_owned())
}

fn env_target(
    explicit: Option<&std::path::Path>,
) -> Result<PathBuf, dx_tool_resolve::ResolveError> {
    match explicit {
        Some(path) => Ok(path.to_owned()),
        None => std::env::var_os("GITHUB_ENV")
            .map(PathBuf::from)
            .ok_or_else(|| dx_tool_resolve::ResolveError::Usage {
                detail: "GITHUB_ENV is not set and neither --env-file nor --print was given"
                    .to_owned(),
            }),
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod main_tests;
