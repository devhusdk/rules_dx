use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;
use dx_error::ErrorKind;

use crate::{
    CommandBazel, RESOLVE_CODE, ResolveError, append_env_file, context_dir, env_line, resolve,
    validate_var,
};

#[derive(Debug, Parser)]
#[command(
    name = "dx tool_resolve",
    bin_name = "dx tool_resolve",
    about = "Resolve one pinned tool label to the absolute artifact path CI exports",
    long_about = "`dx tool_resolve` runs `bazel cquery --output=files` for one pinned tool label, \
takes the last output line as the artifact path, joins an `external/` path with `bazel info output_base` and any other path (a built \
output, which must be built first) with `bazel info execution_root`, \
refuses a missing file, and appends VAR=path to the GitHub environment file so later \
steps read the hermetic tool without host parsing.\n\n--label LABEL names the tool \
(e.g. @rules_dx//core/quality/artifacts:gitleaks); --var NAME names the exported variable (e.g. \
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
    /// Environment file receiving VAR=path instead of `GITHUB_ENV`.
    #[arg(long)]
    env_file: Option<PathBuf>,
    /// Write VAR=path to standard output instead of a file.
    #[arg(long)]
    print: bool,
    /// Extra arguments reaching the inner cquery unchanged.
    #[arg(last = true)]
    bazel_args: Vec<String>,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    let cli = match Cli::try_parse_from(
        std::iter::once("dx tool_resolve").chain(args.iter().map(String::as_str)),
    ) {
        Ok(cli) => cli,
        Err(error) => {
            drop(error.print());
            return if error.use_stderr() {
                ErrorKind::Usage.exit_code()
            } else {
                0
            };
        }
    };
    let github_env = std::env::var_os("GITHUB_ENV");
    let workspace_env = std::env::var_os("BUILD_WORKSPACE_DIRECTORY");
    match run(cli, github_env.as_deref(), workspace_env.as_deref()) {
        Ok(line) => match writeln!(std::io::stdout().lock(), "{line}") {
            Ok(()) => 0,
            Err(error) => {
                dx_error::stdio::eprint_line(format_args!(
                    "{RESOLVE_CODE}: could not write to standard output: {error}"
                ));
                ErrorKind::Failure.exit_code()
            }
        },
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("{error}"));
            ErrorKind::Failure.exit_code()
        }
    }
}

fn run(
    cli: Cli,
    github_env: Option<&OsStr>,
    workspace_env: Option<&OsStr>,
) -> Result<String, ResolveError> {
    validate_var(&cli.var)?;
    let env_file = if cli.print {
        None
    } else {
        Some(env_target(cli.env_file.as_deref(), github_env)?)
    };
    let current = dx_process::current_directory().map_err(|error| ResolveError::Usage {
        detail: format!("could not read the current directory: {error}"),
    })?;
    let dir = context_dir(cli.workspace.as_deref(), workspace_env, &current);
    let bazel = CommandBazel {
        bin: cli.bazel,
        dir,
    };
    let path = resolve(&bazel, &cli.label, &cli.bazel_args)?;
    let line = env_line(&cli.var, &path)?;
    if let Some(env_file) = env_file {
        append_env_file(&env_file, &line).map_err(|error| ResolveError::EnvWrite {
            var: cli.var.clone(),
            path: env_file.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    Ok(line.trim_end().to_owned())
}

fn env_target(
    explicit: Option<&Path>,
    github_env: Option<&OsStr>,
) -> Result<PathBuf, ResolveError> {
    match (explicit, github_env) {
        (Some(path), _) => Ok(path.to_owned()),
        (None, Some(value)) => Ok(PathBuf::from(value)),
        (None, None) => Err(ResolveError::Usage {
            detail: "GITHUB_ENV is not set and neither --env-file nor --print was given".to_owned(),
        }),
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;
