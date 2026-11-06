#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::{fmt::Display, path::PathBuf};

use clap::{error::ErrorKind, Arg, ArgAction, Command};

pub trait ShardKind: 'static {
    const PROGRAM: &'static str;
    const GROUP_FLAG: &'static str;
    const USAGE: &'static str;
    const ENTRY_SHAPE: &'static str;

    type Entry: Clone + Send + Sync + 'static;
    type Shard;
    type Error: Display;

    fn parse_entry(raw: &str) -> Option<Self::Entry>;
    fn shard(producer: String, group: String, entries: Vec<Self::Entry>) -> Self::Shard;
    fn encode(shard: &Self::Shard) -> Result<Vec<u8>, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriterError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Codec(String),
    #[error("{0}")]
    Io(String),
}

fn usage<K: ShardKind>() -> WriterError {
    WriterError::Usage(K::USAGE.to_owned())
}

fn parse_entry_value<K: ShardKind>(raw: &str) -> Result<K::Entry, String> {
    K::parse_entry(raw).ok_or_else(|| format!("bad --entry {raw:?}: want {}", K::ENTRY_SHAPE))
}

fn single(name: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Set)
        .allow_hyphen_values(true)
        .overrides_with(name)
        .value_parser(clap::value_parser!(String))
}

fn command<K: ShardKind>() -> Command {
    Command::new(K::PROGRAM)
        .disable_help_flag(true)
        .arg(single("producer"))
        .arg(single(K::GROUP_FLAG))
        .arg(
            Arg::new("entry")
                .long("entry")
                .action(ArgAction::Append)
                .allow_hyphen_values(true)
                .value_parser(parse_entry_value::<K>),
        )
        .arg(single("output"))
}

fn parse_error<K: ShardKind>(error: clap::Error, args: &[String]) -> String {
    match error.kind() {
        ErrorKind::UnknownArgument => {
            format!(
                "unknown argument {:?}\n{}",
                dx_output::unknown_token(&error, args),
                K::USAGE
            )
        }
        ErrorKind::InvalidValue => K::USAGE.to_owned(),
        ErrorKind::ValueValidation => {
            let raw = dx_output::rejected_value(&error).unwrap_or_default();
            match parse_entry_value::<K>(&raw) {
                Err(message) => message,
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

pub fn run<K: ShardKind>(args: &[String]) -> Result<(), WriterError> {
    let mut matches = command::<K>()
        .try_get_matches_from(std::iter::once(K::PROGRAM).chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| WriterError::Usage(parse_error::<K>(error, args)))?;
    let output = matches.remove_one::<String>("output").map(PathBuf::from);
    let producer = matches
        .remove_one::<String>("producer")
        .ok_or_else(usage::<K>)?;
    let group = matches
        .remove_one::<String>(K::GROUP_FLAG)
        .ok_or_else(usage::<K>)?;
    let entries: Vec<K::Entry> = matches
        .remove_many::<K::Entry>("entry")
        .map(Iterator::collect)
        .unwrap_or_default();
    let bytes = K::encode(&K::shard(producer, group, entries))
        .map_err(|error| WriterError::Codec(error.to_string()))?;
    let output = output.ok_or_else(usage::<K>)?;
    dx_atomic_fs::write_atomic(&output, &bytes)
        .map_err(|error| WriterError::Io(format!("cannot write {}: {error}", output.display())))
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
pub fn main<K: ShardKind>() {
    dx_output::init_diagnostics(false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run::<K>(&args) {
        tracing::error!("{}: {error}", K::PROGRAM);
        std::process::exit(1);
    }
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeKind;

    impl ShardKind for FakeKind {
        const PROGRAM: &'static str = "fake_shard_writer";
        const GROUP_FLAG: &'static str = "group";
        const USAGE: &'static str =
            "usage: fake_shard_writer --producer LABEL --group G --entry A|B [--entry ...] --output OUT";
        const ENTRY_SHAPE: &'static str = "A|B";

        type Entry = (String, String);
        type Shard = (String, String, Vec<(String, String)>);
        type Error = String;

        fn parse_entry(raw: &str) -> Option<Self::Entry> {
            let (left, right) = raw.split_once('|')?;
            (!right.contains('|')).then(|| (left.to_owned(), right.to_owned()))
        }

        fn shard(producer: String, group: String, entries: Vec<Self::Entry>) -> Self::Shard {
            (producer, group, entries)
        }

        fn encode(shard: &Self::Shard) -> Result<Vec<u8>, Self::Error> {
            let (producer, group, entries) = shard;
            if !producer.starts_with("//") {
                return Err(format!("invalid producer {producer:?}"));
            }
            let rendered: Vec<String> = entries.iter().map(|(a, b)| format!("{a}={b}")).collect();
            Ok(format!("{producer} {group} {}", rendered.join(",")).into_bytes())
        }
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("scratch")
    }

    fn out_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("shard.pb")
    }

    fn out(dir: &tempfile::TempDir) -> String {
        out_path(dir).display().to_string()
    }

    fn written(dir: &tempfile::TempDir) -> String {
        std::fs::read_to_string(out_path(dir)).expect("shard written")
    }

    fn minimal(dir: &tempfile::TempDir, extra: &[&str]) -> Vec<String> {
        let mut argv = args(&["--producer=//x:alpha", "--group=rust", "--entry=K|v"]);
        argv.extend(args(extra));
        argv.push(format!("--output={}", out(dir)));
        argv
    }

    fn usage_text() -> WriterError {
        WriterError::Usage(FakeKind::USAGE.to_owned())
    }

    #[test]
    fn a_minimal_shard_is_encoded_and_written() {
        let dir = scratch();
        run::<FakeKind>(&minimal(&dir, &["--entry=L|w"])).expect("minimal shard writes");
        assert_eq!(written(&dir), "//x:alpha rust K=v,L=w");
    }

    #[test]
    fn hyphen_leading_values_are_accepted() {
        let dir = scratch();
        let argv = args(&[
            "--producer",
            "//x:alpha",
            "--group",
            "-rust",
            "--entry",
            "-K|-v",
            "--output",
            &out(&dir),
        ]);
        run::<FakeKind>(&argv).expect("hyphen values write");
        assert_eq!(written(&dir), "//x:alpha -rust -K=-v");
    }

    #[test]
    fn a_bad_entry_names_the_wanted_shape_and_writes_nothing() {
        let dir = scratch();
        let error = run::<FakeKind>(&minimal(&dir, &["--entry=only-one-part"]))
            .expect_err("a one-part entry is rejected");
        assert_eq!(
            error,
            WriterError::Usage("bad --entry \"only-one-part\": want A|B".to_owned())
        );
        assert!(!out_path(&dir).exists(), "a rejected entry writes no shard");
    }

    #[test]
    fn an_unknown_argument_is_echoed_above_the_usage() {
        let dir = scratch();
        let error =
            run::<FakeKind>(&minimal(&dir, &["--nope"])).expect_err("unknown flags are rejected");
        assert_eq!(
            error,
            WriterError::Usage(format!("unknown argument \"--nope\"\n{}", FakeKind::USAGE))
        );
    }

    #[test]
    fn a_flag_without_its_value_prints_usage() {
        for argv in [
            vec!["--producer"],
            vec!["--group"],
            vec!["--entry"],
            vec!["--output"],
        ] {
            let error = run::<FakeKind>(&args(&argv)).expect_err("a valueless flag is rejected");
            assert_eq!(error, usage_text(), "{argv:?}");
        }
    }

    #[test]
    fn a_repeated_flag_keeps_the_last_value() {
        let dir = scratch();
        let first = dir.path().join("first.pb");
        let argv = args(&[
            "--producer=//x:alpha",
            "--producer=//x:beta",
            "--group=rust",
            "--group=python",
            "--entry=K|v",
            &format!("--output={}", first.display()),
            &format!("--output={}", out(&dir)),
        ]);
        run::<FakeKind>(&argv).expect("repeated flags resolve to the last value");
        assert_eq!(written(&dir), "//x:beta python K=v");
        assert!(!first.exists(), "only the last output wins");
    }

    #[test]
    fn a_missing_producer_group_or_output_prints_usage() {
        let dir = scratch();
        let base = format!("--output={}", out(&dir));
        for (argv, missing) in [
            (args(&["--group=rust", "--entry=K|v", &base]), "--producer"),
            (
                args(&["--producer=//x:alpha", "--entry=K|v", &base]),
                "--group",
            ),
            (
                args(&["--producer=//x:alpha", "--group=rust", "--entry=K|v"]),
                "--output",
            ),
        ] {
            let error = run::<FakeKind>(&argv).expect_err("an incomplete shard is rejected");
            assert_eq!(error, usage_text(), "{missing}");
        }
        assert!(
            !out_path(&dir).exists(),
            "an incomplete shard writes no file"
        );
    }

    #[test]
    fn the_codec_error_wins_over_a_missing_output() {
        let error = run::<FakeKind>(&args(&[
            "--producer=not-a-label",
            "--group=rust",
            "--entry=K|v",
        ]))
        .expect_err("a bad producer is rejected");
        assert_eq!(
            error,
            WriterError::Codec("invalid producer \"not-a-label\"".to_owned())
        );
    }

    #[test]
    fn a_codec_error_writes_no_file() {
        let dir = scratch();
        let error = run::<FakeKind>(&minimal(&dir, &["--producer=not-a-label"]))
            .expect_err("a bad producer is rejected");
        assert!(matches!(error, WriterError::Codec(_)), "{error:?}");
        assert!(!out_path(&dir).exists(), "a rejected shard writes no file");
    }

    #[test]
    fn an_unwritable_output_surfaces_the_io_error() {
        let dir = scratch();
        let blocked = dir.path().join("blocked");
        std::fs::create_dir(&blocked).expect("a directory blocks the write");
        let argv = args(&[
            "--producer=//x:alpha",
            "--group=rust",
            "--entry=K|v",
            &format!("--output={}", blocked.display()),
        ]);
        let error = run::<FakeKind>(&argv).expect_err("writing over a directory fails");
        let named = format!("cannot write {}: ", blocked.display());
        assert!(
            matches!(&error, WriterError::Io(detail) if detail.starts_with(&named) && detail.len() > named.len()),
            "want an io error naming the directory, got {error:?}"
        );
        assert!(blocked.is_dir(), "the directory survives");
    }
}
