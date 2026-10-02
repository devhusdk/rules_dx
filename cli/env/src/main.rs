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
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{error::ErrorKind, Parser};
use dx_env::{identity_hex, parse_staged, probe_symlink, refresh, RefreshOptions, RefreshOutcome};

const METADATA_CANDIDATES: &[(&str, &str)] = &[
    ("rules_dx/env/default_tree.metadata.json", "_main"),
    ("_main/env/default_tree.metadata.json", "_main"),
];

fn usage_error(message: &str) -> i32 {
    tracing::error!("dx env: {message}");
    tracing::error!(
        "usage: env [--workspace DIR] [--staged-bin DIR --metadata FILE] [--lock-timeout-ms N]"
    );
    2
}

#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "workspace")]
    workspace: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "staged_bin")]
    staged_bin: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "metadata")]
    metadata: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "lock_timeout_ms")]
    lock_timeout_ms: Option<String>,
    #[arg(long = "help", short = 'h', action = clap::ArgAction::SetTrue)]
    help: bool,
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

fn parse_args(args: &[String]) -> Result<Cli, String> {
    Cli::try_parse_from(std::iter::once("env").chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| parse_error(error, args))
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(&args) {
        Ok(cli) => cli,
        Err(message) => return usage_error(&message),
    };
    if cli.help {
        return usage_error("install the staged environment tree into .dx/bin");
    }
    let (workspace, staged_bin, metadata, lock_timeout_ms) = (
        cli.workspace,
        cli.staged_bin,
        cli.metadata,
        cli.lock_timeout_ms,
    );
    if staged_bin.is_none() != metadata.is_none() {
        return usage_error("--staged-bin and --metadata must be passed together");
    }
    let lock_timeout = match lock_timeout_ms {
        None => dx_env::LOCK_TIMEOUT,
        Some(millis) => match millis.parse::<u64>() {
            Ok(millis) => Duration::from_millis(millis),
            Err(_) => return usage_error("--lock-timeout-ms must be a non-negative integer"),
        },
    };
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let start = match workspace {
        Some(dir) => PathBuf::from(dir),
        None => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            dx_process::workspace_start(&cwd)
        }
    };
    let (staged_bin, metadata) = match (staged_bin, metadata) {
        (Some(bin), Some(meta)) => (PathBuf::from(bin), PathBuf::from(meta)),
        _ => match locate_default_tree() {
            Some(paths) => paths,
            None => {
                tracing::error!("dx env: default tree not found in runfiles; pass --staged-bin and --metadata explicitly");
                return 1;
            }
        },
    };
    let options = RefreshOptions {
        workspace_root: start,
        staged_bin,
        staged_metadata: metadata.clone(),
        os: std::env::consts::OS,
        lock_timeout,
    };
    let tools = match std::fs::read_to_string(&metadata) {
        Ok(text) => match parse_staged(&text) {
            Ok(tools) => tools,
            Err(error) => {
                tracing::error!("dx env: {error}");
                return 1;
            }
        },
        Err(error) => {
            tracing::error!(
                "dx env: cannot read staged metadata {}: {error}",
                metadata.display()
            );
            return 1;
        }
    };
    match refresh(&options, &probe_symlink) {
        Ok(RefreshOutcome::AlreadyCurrent) => {
            println!("dx env: already current (.dx/bin {})", identity_hex(&tools));
            0
        }
        Ok(RefreshOutcome::InstalledFresh) => {
            println!(
                "dx env: installed {} tool(s) (.dx/bin {})",
                tools.len(),
                identity_hex(&tools)
            );
            0
        }
        Ok(RefreshOutcome::InstalledReplacement) => {
            println!(
                "dx env: replaced managed tree with {} tool(s) (.dx/bin {})",
                tools.len(),
                identity_hex(&tools)
            );
            0
        }
        Err(error) => {
            tracing::error!("dx env: {error}");
            1
        }
    }
}

fn locate_default_tree() -> Option<(PathBuf, PathBuf)> {
    let runfiles = runfiles::Runfiles::create().ok()?;
    for (path, source_repo) in METADATA_CANDIDATES {
        if let Some(metadata) = runfiles.rlocation_from(Path::new(path), source_repo) {
            if metadata.is_file() {
                if let Some(parent) = metadata.parent() {
                    let staged_bin = parent.join(dx_env::BIN_DIR_NAME);
                    if staged_bin.is_dir() {
                        return Some((staged_bin, metadata));
                    }
                }
            }
        }
    }
    None
}

fn main() {
    dx_output::init_diagnostics(false);
    std::process::exit(run());
}
