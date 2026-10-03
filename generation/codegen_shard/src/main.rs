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
use codegen_shard::{
    encode_validated,
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

// LCOV_EXCL_START - reason: clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
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
// LCOV_EXCL_STOP - reason: end clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

fn parse_error(error: clap::Error, args: &[String]) -> WriterError {
    match error.kind() {
        ErrorKind::UnknownArgument => WriterError::Usage(format!(
            "unknown argument {:?}\n{}",
            dx_output::unknown_token(&error, args),
            usage()
        )),
        ErrorKind::InvalidValue => WriterError::Usage(usage()),
        ErrorKind::ValueValidation => {
            let raw = dx_output::rejected_value(&error).unwrap_or_default();
            match parse_entry_value(&raw) {
                Err(legacy) => legacy,
                // LCOV_EXCL_START - reason: clap reports the value it rejected, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                Ok(_) => WriterError::Usage(dx_output::first_line(&error)),
                // LCOV_EXCL_STOP - reason: end clap reports the value it rejected, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
            }
        }
        // LCOV_EXCL_START - reason: clap kinds this command cannot raise, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        _ => WriterError::Usage(dx_output::first_line(&error)),
        // LCOV_EXCL_STOP - reason: end clap kinds this command cannot raise, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    }
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
    let bytes = encode_validated(&shard).map_err(WriterError::Codec)?;
    dx_atomic_fs::write_atomic(&output.ok_or_else(|| WriterError::Usage(usage()))?, &bytes)
        .map_err(WriterError::Io)
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    dx_output::init_diagnostics(false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        tracing::error!("codegen_shard_writer: {error}");
        std::process::exit(1);
    }
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use codegen_shard::decode_validated;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("scratch")
    }

    fn out_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("shard.dxcodegen.pb")
    }

    fn out(dir: &tempfile::TempDir) -> String {
        out_path(dir).display().to_string()
    }

    fn written(dir: &tempfile::TempDir) -> DxCodegenShard {
        let bytes = std::fs::read(out(dir)).expect("shard written");
        decode_validated(&bytes).expect("written shard decodes")
    }

    fn minimal(dir: &tempfile::TempDir, extra: &[&str]) -> Vec<String> {
        let mut argv = args(&[
            "--producer=//gen:alpha",
            "--language=rust",
            "--entry=src/a.rs|src|a",
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
        assert_eq!(shard.producer, "//gen:alpha");
        assert_eq!(shard.language, "rust");
        assert_eq!(shard.entries.len(), 1);
        assert_eq!(shard.entries[0].logical_path, "src/a.rs");
        assert_eq!(shard.entries[0].import_root, "src");
        assert_eq!(shard.entries[0].namespace, "a");
        assert_eq!(shard.entries[0].exec_path, "");
        assert_eq!(shard.entries[0].replaces, "");
        assert!(shard.entries[0].read_only);
    }

    #[test]
    fn every_entry_arity_binds_its_own_fields() {
        for (raw, logical_path, namespace, exec_path, replaces) in [
            ("src/one.rs|src|one", "src/one.rs", "one", "", ""),
            ("src/b.rs|src|b|out/b.rs", "src/b.rs", "b", "out/b.rs", ""),
            (
                "src/c.rs|src|c|out/c.rs|src/c.rs",
                "src/c.rs",
                "c",
                "out/c.rs",
                "src/c.rs",
            ),
        ] {
            let dir = scratch();
            run(&minimal(&dir, &[&format!("--entry={raw}")]))
                .unwrap_or_else(|error| panic!("entry {raw:?} must write: {error}"));
            let entry = written(&dir)
                .entries
                .into_iter()
                .find(|entry| entry.logical_path == logical_path)
                .unwrap_or_else(|| panic!("entry {raw:?} must survive the round trip"));
            assert_eq!(entry.import_root, "src");
            assert_eq!(entry.namespace, namespace);
            assert_eq!(entry.exec_path, exec_path);
            assert_eq!(entry.replaces, replaces);
        }
    }

    #[test]
    fn a_bad_entry_arity_names_the_wanted_shape() {
        let dir = scratch();
        let error = run(&minimal(&dir, &["--entry=only-one-part"]))
            .expect_err("a one-part entry is rejected");
        assert!(
            matches!(&error, WriterError::BadEntry { raw } if raw == "only-one-part"),
            "want BadEntry, got {error:?}"
        );
        assert_eq!(
            error.to_string(),
            "bad --entry \"only-one-part\": want LOGICAL_PATH|IMPORT_ROOT|NAMESPACE[|EXEC_PATH[|REPLACES]]"
        );
        assert!(!out_path(&dir).exists(), "a rejected entry writes no shard");
    }

    #[test]
    fn an_unknown_argument_is_echoed_above_the_usage() {
        let dir = scratch();
        let error = run(&minimal(&dir, &["--nope"])).expect_err("unknown flags are rejected");
        assert_eq!(
            error.to_string(),
            format!("unknown argument \"--nope\"\n{}", usage())
        );
    }

    #[test]
    fn a_flag_without_its_value_prints_usage() {
        for argv in [vec!["--producer"], vec!["--entry"], vec!["--output"]] {
            let error = run(&args(&argv)).expect_err("a valueless flag is rejected");
            assert_eq!(error.to_string(), usage(), "argv {argv:?}");
        }
    }

    #[test]
    fn a_repeated_flag_keeps_the_last_value() {
        let dir = scratch();
        let first = dir.path().join("first.pb").display().to_string();
        let second = out(&dir);
        let argv = args(&[
            "--producer=//gen:alpha",
            "--producer=//gen:beta",
            "--language=rust",
            "--language=python",
            "--entry=src/a.rs|src|a",
            &format!("--output={first}"),
            &format!("--output={second}"),
        ]);
        run(&argv).expect("repeated flags resolve to the last value");
        let shard = written(&dir);
        assert_eq!(shard.producer, "//gen:beta");
        assert_eq!(shard.language, "python");
        assert!(
            !std::path::Path::new(&first).exists(),
            "only the last output wins"
        );
    }

    #[test]
    fn a_missing_producer_language_or_output_prints_usage() {
        let dir = scratch();
        let output = out(&dir);
        let base = format!("--output={output}");
        for (argv, missing) in [
            (
                args(&["--language=rust", "--entry=src/a.rs|src|a", &base]),
                "--producer",
            ),
            (
                args(&["--producer=//gen:alpha", "--entry=src/a.rs|src|a", &base]),
                "--language",
            ),
            (
                args(&[
                    "--producer=//gen:alpha",
                    "--language=rust",
                    "--entry=src/a.rs|src|a",
                ]),
                "--output",
            ),
        ] {
            let error = run(&argv).expect_err("an incomplete shard is rejected");
            assert_eq!(
                error.to_string(),
                usage(),
                "{missing} must be named by usage"
            );
        }
    }

    #[test]
    fn an_invalid_shard_surfaces_the_codec_error() {
        let dir = scratch();
        let error = run(&minimal(
            &dir,
            &["--producer=not-a-label", "--entry=src/b.rs|src|b"],
        ))
        .expect_err("a producer without // or @ is rejected");
        assert!(
            matches!(&error, WriterError::Codec(codegen_shard::Error::BadProducer { value }) if value == "not-a-label"),
            "want the codec's producer error, got {error:?}"
        );
        assert_eq!(
            error.to_string(),
            "invalid producer \"not-a-label\": want a // or @ label"
        );
        assert!(!out_path(&dir).exists(), "a rejected shard writes no file");
    }

    #[test]
    fn duplicate_entries_are_rejected_by_the_codec() {
        let dir = scratch();
        let error = run(&minimal(&dir, &["--entry=src/a.rs|src|other"]))
            .expect_err("a duplicate logical path is rejected");
        assert!(
            matches!(&error, WriterError::Codec(codegen_shard::Error::DuplicateLogicalPath { path, .. }) if path == "src/a.rs"),
            "want the duplicate error, got {error:?}"
        );
    }

    #[test]
    fn an_unwritable_output_surfaces_the_io_error() {
        let dir = scratch();
        let blocked = dir.path().join("blocked");
        std::fs::create_dir(&blocked).expect("a directory blocks the write");
        let mut argv = args(&[
            "--producer=//gen:alpha",
            "--language=rust",
            "--entry=src/a.rs|src|a",
        ]);
        argv.push(format!("--output={}", blocked.display()));
        let error = run(&argv).expect_err("writing over a directory fails");
        assert!(
            matches!(&error, WriterError::Io(_)),
            "want an io error, got {error:?}"
        );
    }
}
