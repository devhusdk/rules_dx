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
use std::path::PathBuf;

use clap::{error::ErrorKind, Parser};
use env_shard::{
    decode_validated, encode_validated,
    proto::{DxEnvEntry, DxEnvShard},
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvShardError {
    #[error("{message}")]
    Args { message: String },
    #[error("bad --entry {raw:?}: want KEY|VALUE[|EXEC_PATH]")]
    BadEntry { raw: String },
    #[error("{message}")]
    Usage { message: String },
    #[error("{detail}")]
    Codec { detail: String },
    #[error("{detail}")]
    Io { detail: String },
}

fn usage() -> String {
    "usage: env_shard_writer --producer LABEL --integration LANG --entry KEY|VALUE[|EXEC] [--entry ...] --output OUT".into()
}

#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "producer")]
    producer: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "integration")]
    integration: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_parser = parse_entry_value)]
    entry: Vec<DxEnvEntry>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "output")]
    output: Option<String>,
}

fn parse_error(error: clap::Error, args: &[String]) -> String {
    match error.kind() {
        ErrorKind::UnknownArgument => {
            format!(
                "unknown argument {:?}\n{}",
                dx_output::unknown_token(&error, args),
                usage()
            )
        }
        ErrorKind::InvalidValue => usage(),
        ErrorKind::ValueValidation => {
            let raw = dx_output::rejected_value(&error).unwrap_or_default();
            match parse_entry_value(&raw) {
                Err(legacy) => legacy,
                Ok(_) => dx_output::first_line(&error),
            }
        }
        _ => dx_output::first_line(&error),
    }
}

fn parse_args(args: &[String]) -> Result<Cli, EnvShardError> {
    Cli::try_parse_from(
        std::iter::once("env_shard_writer").chain(args.iter().map(|arg| arg as &str)),
    )
    .map_err(|error| EnvShardError::Args {
        message: parse_error(error, args),
    })
}

fn parse_entry_value(raw: &str) -> Result<DxEnvEntry, String> {
    let parts: Vec<&str> = raw.split('|').collect();
    match parts.len() {
        2 => Ok(DxEnvEntry {
            key: parts[0].into(),
            value: parts[1].into(),
            exec_path: String::new(),
        }),
        3 => Ok(DxEnvEntry {
            key: parts[0].into(),
            value: parts[1].into(),
            exec_path: parts[2].into(),
        }),
        _ => Err(EnvShardError::BadEntry {
            raw: raw.to_owned(),
        }
        .to_string()),
    }
}

fn run(args: &[String]) -> Result<(), EnvShardError> {
    let cli = parse_args(args)?;
    let output = cli.output.map(PathBuf::from);
    let shard = DxEnvShard {
        producer: cli
            .producer
            .ok_or_else(|| EnvShardError::Usage { message: usage() })?,
        integration: cli
            .integration
            .ok_or_else(|| EnvShardError::Usage { message: usage() })?,
        entries: cli.entry,
    };
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let bytes = encode_validated(&shard).map_err(|error| EnvShardError::Codec {
        detail: error.to_string(),
    })?;
    decode_validated(&bytes).map_err(|error| EnvShardError::Codec {
        detail: error.to_string(),
    })?;
    let output = output.ok_or_else(|| EnvShardError::Usage { message: usage() })?;
    dx_atomic_fs::write_atomic(&output, &bytes).map_err(|error| EnvShardError::Io {
        detail: error.to_string(),
    })
}

fn main() {
    dx_output::init_diagnostics(false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        tracing::error!("env_shard_writer: {error}");
        std::process::exit(1);
    }
}
