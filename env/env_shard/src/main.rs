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

use clap::{error::ErrorKind, Parser};
use env_shard::{
    encode_validated,
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

// LCOV_EXCL_START - reason: clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
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
// LCOV_EXCL_STOP - reason: end clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

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
                // LCOV_EXCL_START - reason: clap reports the value it rejected, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                Ok(_) => dx_output::first_line(&error),
                // LCOV_EXCL_STOP - reason: end clap reports the value it rejected, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
            }
        }
        // LCOV_EXCL_START - reason: clap kinds this command cannot raise, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        _ => dx_output::first_line(&error),
        // LCOV_EXCL_STOP - reason: end clap kinds this command cannot raise, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
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
    let bytes = encode_validated(&shard).map_err(|error| EnvShardError::Codec {
        detail: error.to_string(),
    })?;
    let output = output.ok_or_else(|| EnvShardError::Usage { message: usage() })?;
    dx_atomic_fs::write_atomic(&output, &bytes).map_err(|error| EnvShardError::Io {
        detail: error.to_string(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    dx_output::init_diagnostics(false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        tracing::error!("env_shard_writer: {error}");
        std::process::exit(1);
    }
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use env_shard::decode_validated;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("scratch")
    }

    fn out_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("shard.dxenv.pb")
    }

    fn out(dir: &tempfile::TempDir) -> String {
        out_path(dir).display().to_string()
    }

    fn written(dir: &tempfile::TempDir) -> DxEnvShard {
        let bytes = std::fs::read(out(dir)).expect("shard written");
        decode_validated(&bytes).expect("written shard decodes")
    }

    fn minimal(dir: &tempfile::TempDir, extra: &[&str]) -> Vec<String> {
        let mut argv = args(&[
            "--producer=//env:alpha",
            "--integration=rust",
            "--entry=TOKEN|secret",
        ]);
        argv.extend(args(extra));
        argv.push(format!("--output={}", out(dir)));
        argv
    }

    #[test]
    fn a_minimal_shard_round_trips_through_the_writer() {
        let dir = scratch();
        run(&minimal(&dir, &[])).expect("minimal shard writes");
        let shard = written(&dir);
        assert_eq!(shard.producer, "//env:alpha");
        assert_eq!(shard.integration, "rust");
        assert_eq!(shard.entries.len(), 1);
        assert_eq!(shard.entries[0].key, "TOKEN");
        assert_eq!(shard.entries[0].value, "secret");
        assert_eq!(shard.entries[0].exec_path, "");
    }

    #[test]
    fn both_entry_arities_bind_their_own_fields() {
        for (raw, key, value, exec_path) in [
            ("ONE|1", "ONE", "1", ""),
            ("TWO|2|out/two.txt", "TWO", "2", "out/two.txt"),
        ] {
            let dir = scratch();
            run(&minimal(&dir, &[&format!("--entry={raw}")]))
                .unwrap_or_else(|error| panic!("entry {raw:?} must write: {error}"));
            let entry = written(&dir)
                .entries
                .into_iter()
                .find(|entry| entry.key == key)
                .unwrap_or_else(|| panic!("entry {raw:?} must survive the round trip"));
            assert_eq!(entry.value, value);
            assert_eq!(entry.exec_path, exec_path);
        }
    }

    #[test]
    fn a_bad_entry_arity_names_the_wanted_shape() {
        let dir = scratch();
        let error = run(&minimal(&dir, &["--entry=only-one-part"]))
            .expect_err("a one-part entry is rejected");
        assert_eq!(
            error,
            EnvShardError::Args {
                message: "bad --entry \"only-one-part\": want KEY|VALUE[|EXEC_PATH]".to_owned()
            }
        );
        assert_eq!(
            error.to_string(),
            "bad --entry \"only-one-part\": want KEY|VALUE[|EXEC_PATH]"
        );
        assert!(!out_path(&dir).exists(), "a rejected entry writes no shard");
    }

    #[test]
    fn an_unknown_argument_is_echoed_above_the_usage() {
        let dir = scratch();
        let error = run(&minimal(&dir, &["--nope"])).expect_err("unknown flags are rejected");
        assert_eq!(
            error,
            EnvShardError::Args {
                message: format!("unknown argument \"--nope\"\n{}", usage())
            }
        );
    }

    #[test]
    fn a_flag_without_its_value_prints_usage() {
        for argv in [vec!["--producer"], vec!["--entry"], vec!["--output"]] {
            let error = run(&args(&argv)).expect_err("a valueless flag is rejected");
            assert_eq!(error, EnvShardError::Args { message: usage() }, "{argv:?}");
        }
    }

    #[test]
    fn a_repeated_flag_keeps_the_last_value() {
        let dir = scratch();
        let first = dir.path().join("first.pb").display().to_string();
        let second = out(&dir);
        let argv = args(&[
            "--producer=//env:alpha",
            "--producer=//env:beta",
            "--integration=rust",
            "--integration=python",
            "--entry=TOKEN|secret",
            &format!("--output={first}"),
            &format!("--output={second}"),
        ]);
        run(&argv).expect("repeated flags resolve to the last value");
        let shard = written(&dir);
        assert_eq!(shard.producer, "//env:beta");
        assert_eq!(shard.integration, "python");
        assert!(
            !std::path::Path::new(&first).exists(),
            "only the last output wins"
        );
    }

    #[test]
    fn a_missing_producer_integration_or_output_prints_usage() {
        let dir = scratch();
        let base = format!("--output={}", out(&dir));
        for (argv, missing) in [
            (
                args(&["--integration=rust", "--entry=TOKEN|secret", &base]),
                "--producer",
            ),
            (
                args(&["--producer=//env:alpha", "--entry=TOKEN|secret", &base]),
                "--integration",
            ),
            (
                args(&[
                    "--producer=//env:alpha",
                    "--integration=rust",
                    "--entry=TOKEN|secret",
                ]),
                "--output",
            ),
        ] {
            let error = run(&argv).expect_err("an incomplete shard is rejected");
            assert_eq!(
                error,
                EnvShardError::Usage { message: usage() },
                "{missing}"
            );
        }
    }

    #[test]
    fn an_invalid_shard_surfaces_the_codec_error() {
        let dir = scratch();
        let error = run(&minimal(
            &dir,
            &["--producer=not-a-label", "--entry=OTHER|1"],
        ))
        .expect_err("a producer without // or @ is rejected");
        assert_eq!(
            error,
            EnvShardError::Codec {
                detail: "invalid producer \"not-a-label\": want a // or @ label".to_owned()
            }
        );
        assert!(!out_path(&dir).exists(), "a rejected shard writes no file");
    }

    #[test]
    fn duplicate_keys_are_rejected_by_the_codec() {
        let dir = scratch();
        let error =
            run(&minimal(&dir, &["--entry=TOKEN|other"])).expect_err("a duplicate key is rejected");
        assert_eq!(
            error,
            EnvShardError::Codec {
                detail: "duplicate key for //env:alpha \"TOKEN\"".to_owned()
            }
        );
    }

    #[test]
    fn an_unwritable_output_surfaces_the_io_error() {
        let dir = scratch();
        let blocked = dir.path().join("blocked");
        std::fs::create_dir(&blocked).expect("a directory blocks the write");
        let mut argv = args(&[
            "--producer=//env:alpha",
            "--integration=rust",
            "--entry=TOKEN|secret",
        ]);
        argv.push(format!("--output={}", blocked.display()));
        let error = run(&argv).expect_err("writing over a directory fails");
        assert!(
            matches!(&error, EnvShardError::Io { detail } if detail.contains("Is a directory")),
            "want an io error naming the directory, got {error:?}"
        );
    }
}
