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
use clap::{error::ErrorKind, Parser};
use quality_evaluator::{evaluate, parse_threshold, Threshold};
use quality_result::decode_validated;

#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "result")]
    result: Option<String>,
    #[arg(
        long = "fail_on",
        allow_hyphen_values = true,
        overrides_with = "fail_on",
        value_parser = parse_fail_on
    )]
    fail_on: Option<Threshold>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "output")]
    output: Option<String>,
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
            if flag == "--fail_on" {
                if let Some(raw) = rejected_value(&error) {
                    if let Err(legacy) = parse_threshold(&raw) {
                        return legacy.to_string();
                    }
                }
            }
            format!("missing value for {flag}")
        }
        ErrorKind::ValueValidation => {
            let raw = rejected_value(&error).unwrap_or_default();
            match parse_threshold(&raw) {
                Err(legacy) => legacy.to_string(),
                Ok(_) => dx_output::first_line(&error),
            }
        }
        _ => dx_output::first_line(&error),
    }
}

fn rejected_value(error: &clap::Error) -> Option<String> {
    dx_output::rejected_value(error)
}

fn parse_fail_on(raw: &str) -> Result<Threshold, String> {
    parse_threshold(raw).map_err(|error| error.to_string())
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    Cli::try_parse_from(
        std::iter::once("quality_evaluator").chain(args.iter().map(|arg| arg as &str)),
    )
    .map_err(|error| parse_error(error, args))
}

fn main() {
    dx_output::init_diagnostics(false);
    if let Err(message) = run() {
        tracing::error!("quality_evaluator: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse_args(&args)?;
    let result_path = cli.result.ok_or("--result is required")?;
    let threshold = cli.fail_on.ok_or("--fail_on is required")?;
    let output = cli.output.ok_or("--output is required")?;
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let bytes =
        std::fs::read(&result_path).map_err(|e| format!("cannot read {result_path:?}: {e}"))?;
    let result =
        decode_validated(&bytes).map_err(|e| format!("invalid result {result_path:?}: {e:?}"))?;
    let evaluation = evaluate(&result, threshold);
    if !evaluation.passed {
        return Err(evaluation.reasons.join("; "));
    }
    let marker = format!(
        "validated {} {} fail_on={}\n",
        result.producer,
        result.capability,
        threshold.name()
    );
    dx_atomic_fs::write_atomic(std::path::Path::new(&output), marker.as_bytes())
        .map_err(|e| format!("cannot write {output:?}: {e}"))?;
    Ok(())
}
