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
use codegen_shard::{
    decode_validated, encode_validated,
    proto::{DxCodegenEntry, DxCodegenShard},
};

fn usage() -> String {
    "usage: codegen_shard_writer --producer LABEL --language LANG --entry LOGICAL|ROOT|NAMESPACE[|EXEC] [--entry ...] --output OUT".into()
}

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error("bad --entry {raw:?}: want LOGICAL_PATH|IMPORT_ROOT|NAMESPACE[|EXEC_PATH[|REPLACES]]")]
    BadEntry { raw: String },
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Codec(#[source] codegen_shard::Error),
    #[error("{0}")]
    Io(#[source] std::io::Error),
}

#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "producer")]
    producer: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "language")]
    language: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_parser = parse_entry_value)]
    entry: Vec<DxCodegenEntry>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "output")]
    output: Option<String>,
}

fn invalid_token(error: &clap::Error) -> String {
    dx_output::invalid_token(error)
}

fn parse_error(error: clap::Error, args: &[String]) -> WriterError {
    let token = invalid_token(&error);
    match error.kind() {
        ErrorKind::UnknownArgument => {
            let echoed = dx_output::recover_unknown_token(args, &token);
            WriterError::Usage(format!("unknown argument {echoed:?}\n{}", usage()))
        }
        ErrorKind::InvalidValue => WriterError::Usage(usage()),
        ErrorKind::ValueValidation => {
            let raw = rejected_value(&error).unwrap_or_default();
            match parse_entry_value(&raw) {
                Err(legacy) => legacy,
                Ok(_) => WriterError::Usage(dx_output::first_line(&error)),
            }
        }
        _ => WriterError::Usage(dx_output::first_line(&error)),
    }
}

fn rejected_value(error: &clap::Error) -> Option<String> {
    dx_output::rejected_value(error)
}

fn parse_args(args: &[String]) -> Result<Cli, WriterError> {
    Cli::try_parse_from(
        std::iter::once("codegen_shard_writer").chain(args.iter().map(|arg| arg as &str)),
    )
    .map_err(|error| parse_error(error, args))
}

fn parse_entry_value(raw: &str) -> Result<DxCodegenEntry, WriterError> {
    let parts: Vec<&str> = raw.split('|').collect();
    match parts.len() {
        3 => Ok(DxCodegenEntry {
            logical_path: parts[0].into(),
            import_root: parts[1].into(),
            namespace: parts[2].into(),
            read_only: true,
            exec_path: String::new(),
            replaces: String::new(),
        }),
        4 => Ok(DxCodegenEntry {
            logical_path: parts[0].into(),
            import_root: parts[1].into(),
            namespace: parts[2].into(),
            read_only: true,
            exec_path: parts[3].into(),
            replaces: String::new(),
        }),
        5 => Ok(DxCodegenEntry {
            logical_path: parts[0].into(),
            import_root: parts[1].into(),
            namespace: parts[2].into(),
            read_only: true,
            exec_path: parts[3].into(),
            replaces: parts[4].into(),
        }),
        _ => Err(WriterError::BadEntry {
            raw: raw.to_owned(),
        }),
    }
}

fn run(args: &[String]) -> Result<(), WriterError> {
    let cli = parse_args(args)?;
    let output = cli.output.map(PathBuf::from);
    let shard = DxCodegenShard {
        producer: cli.producer.ok_or_else(|| WriterError::Usage(usage()))?,
        language: cli.language.ok_or_else(|| WriterError::Usage(usage()))?,
        entries: cli.entry,
    };
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let bytes = encode_validated(&shard).map_err(WriterError::Codec)?;
    decode_validated(&bytes).map_err(WriterError::Codec)?;
    dx_atomic_fs::write_atomic(&output.ok_or_else(|| WriterError::Usage(usage()))?, &bytes)
        .map_err(WriterError::Io)
}

fn main() {
    dx_output::init_diagnostics(false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        tracing::error!("codegen_shard_writer: {error}");
        std::process::exit(1);
    }
}
