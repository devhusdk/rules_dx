#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use clap::{error::ErrorKind, Parser};
use quality_evaluator::{evaluate, parse_threshold, Threshold};
use quality_result::decode_validated;

// LCOV_EXCL_START - reason: clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
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
// LCOV_EXCL_STOP - reason: end clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

fn parse_error(error: clap::Error, args: &[String]) -> String {
    match error.kind() {
        ErrorKind::UnknownArgument => {
            format!("unknown flag {:?}", dx_output::unknown_token(&error, args))
        }
        ErrorKind::InvalidValue => {
            format!(
                "missing value for {}",
                dx_output::missing_value_flag(&error)
            )
        }
        ErrorKind::ValueValidation => {
            let raw = dx_output::rejected_value(&error).unwrap_or_default();
            match parse_threshold(&raw) {
                Err(legacy) => legacy.to_string(),
                Ok(_) => dx_output::first_line(&error),
            }
        }
        _ => dx_output::first_line(&error),
    }
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

fn run_from(args: &[String]) -> Result<(), String> {
    let cli = parse_args(args)?;
    let result_path = cli.result.ok_or("--result is required")?;
    let threshold = cli.fail_on.ok_or("--fail_on is required")?;
    let output = cli.output.ok_or("--output is required")?;
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
        .map_err(|e| format!("cannot write {output:?}: {e}"))
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    dx_output::init_diagnostics(false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = run_from(&args) {
        tracing::error!("quality_evaluator: {message}");
        std::process::exit(1);
    }
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use quality_result::proto::{
        Capability, Convergence, Diagnostic, QualityResult, Severity, Stage,
    };

    fn clean() -> QualityResult {
        QualityResult {
            schema_major: quality_result::SCHEMA_MAJOR,
            schema_minor: quality_result::SCHEMA_MINOR,
            convergence: Convergence::Stable as i32,
            producer: "//quality:lint".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "lint-a".to_owned(),
                class_ids: vec!["rust".to_owned()],
                source_paths: vec!["src/lib.rs".to_owned()],
            }],
            completed_rounds: 1,
            ..Default::default()
        }
    }

    fn encoded(result: &QualityResult) -> Vec<u8> {
        quality_result::encode_validated(result).expect("fixture encodes")
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("fixture writes");
        path.display().to_string()
    }

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("scratch")
    }

    #[test]
    fn a_clean_result_writes_the_marker() {
        let dir = scratch();
        let result = write(&dir, "result.bin", &encoded(&clean()));
        let out = dir.path().join("marker.txt");
        let argv = args(&[
            "--result",
            &result,
            "--fail_on=error",
            "--output",
            &out.display().to_string(),
        ]);
        run_from(&argv).expect("clean result passes");
        assert_eq!(
            std::fs::read_to_string(&out).expect("marker written"),
            "validated //quality:lint 1 fail_on=error\n"
        );
    }

    #[test]
    fn every_threshold_names_itself_in_the_marker() {
        for threshold in ["info", "warning", "error"] {
            let dir = scratch();
            let result = write(&dir, "result.bin", &encoded(&clean()));
            let out = dir.path().join("marker.txt");
            let argv = args(&[
                "--result",
                &result,
                &format!("--fail_on={threshold}"),
                "--output",
                &out.display().to_string(),
            ]);
            run_from(&argv).expect("clean result passes every threshold");
            assert_eq!(
                std::fs::read_to_string(&out).expect("marker written"),
                format!("validated //quality:lint 1 fail_on={threshold}\n")
            );
        }
    }

    #[test]
    fn a_failing_result_names_every_reason_and_writes_no_marker() {
        let dir = scratch();
        let mut result = clean();
        result.convergence = Convergence::Oscillation as i32;
        result.initial_diagnostics.push(Diagnostic {
            severity: Severity::Error as i32,
            message: "synthetic".to_owned(),
            tool_id: "lint-a".to_owned(),
            path: "quality/testdata/dirty.py".to_owned(),
            start_byte: Some(0),
            end_byte: Some(3),
            ..Default::default()
        });
        let result = write(&dir, "result.bin", &encoded(&result));
        let out = dir.path().join("marker.txt");
        let argv = args(&[
            "--result",
            &result,
            "--fail_on=error",
            "--output",
            &out.display().to_string(),
        ]);
        let error = run_from(&argv).expect_err("oscillation fails");
        assert!(error.starts_with("convergence is "), "{error}");
        assert!(error.contains("synthetic"), "{error}");
        assert!(!out.exists(), "a failing result writes no marker");
    }

    #[test]
    fn an_unreadable_and_an_invalid_result_are_named_separately() {
        let dir = scratch();
        let out = dir.path().join("marker.txt");
        let missing = dir.path().join("absent.bin").display().to_string();
        let unreadable = args(&[
            "--result",
            &missing,
            "--fail_on=error",
            "--output",
            &out.display().to_string(),
        ]);
        assert!(
            run_from(&unreadable)
                .expect_err("missing result")
                .starts_with("cannot read "),
            "a missing result names the read"
        );

        let garbage = write(&dir, "garbage.bin", b"not a result");
        let invalid = args(&[
            "--result",
            &garbage,
            "--fail_on=error",
            "--output",
            &out.display().to_string(),
        ]);
        assert!(
            run_from(&invalid)
                .expect_err("garbage result")
                .starts_with("invalid result "),
            "garbage names the decode"
        );
        assert!(!out.exists(), "neither case writes a marker");
    }

    #[test]
    fn an_unwritable_output_names_the_write() {
        let dir = scratch();
        let result = write(&dir, "result.bin", &encoded(&clean()));
        let out = dir.path().join("occupied");
        std::fs::create_dir(&out).expect("a directory blocks the marker path");
        let argv = args(&[
            "--result",
            &result,
            "--fail_on=error",
            "--output",
            &out.display().to_string(),
        ]);
        assert!(
            run_from(&argv)
                .expect_err("occupied path")
                .starts_with("cannot write "),
            "an occupied output path names the write"
        );
    }

    #[test]
    fn each_required_flag_is_named_when_missing() {
        let dir = scratch();
        let result = write(&dir, "result.bin", &encoded(&clean()));
        for (argv, expected) in [
            (args(&[]), "--result is required"),
            (args(&["--fail_on=error"]), "--result is required"),
            (args(&["--result", &result]), "--fail_on is required"),
            (
                args(&["--result", &result, "--fail_on=error"]),
                "--output is required",
            ),
        ] {
            assert_eq!(
                run_from(&argv).expect_err("missing flag"),
                expected,
                "argv: {argv:?}"
            );
        }
    }

    #[test]
    fn the_last_of_a_repeated_flag_wins() {
        let dir = scratch();
        let result = write(&dir, "result.bin", &encoded(&clean()));
        let out = dir.path().join("marker.txt");
        let argv = args(&[
            "--result",
            &result,
            "--fail_on=info",
            "--fail_on=error",
            "--output",
            &out.display().to_string(),
        ]);
        run_from(&argv).expect("the last value wins");
        assert!(
            std::fs::read_to_string(&out)
                .expect("marker written")
                .ends_with("fail_on=error\n"),
            "the later --fail_on replaces the earlier one"
        );
    }

    #[test]
    fn parse_error_names_the_unknown_token() {
        for (argv, expected) in [
            (args(&["--bogus"]), "unknown flag \"--bogus\""),
            (args(&["junk"]), "unknown flag \"junk\""),
            (args(&["--help"]), "unknown flag \"--help\""),
        ] {
            assert_eq!(
                run_from(&argv).expect_err("unknown flag"),
                expected,
                "argv: {argv:?}"
            );
        }
    }

    #[test]
    fn parse_error_names_the_missing_value_flag() {
        for (argv, expected) in [
            (args(&["--result"]), "missing value for --result"),
            (args(&["--output"]), "missing value for --output"),
            (args(&["--fail_on"]), "missing value for --fail_on"),
        ] {
            assert_eq!(
                run_from(&argv).expect_err("missing value"),
                expected,
                "argv: {argv:?}"
            );
        }
    }

    #[test]
    fn parse_error_prefers_the_threshold_message_for_a_rejected_fail_on() {
        for (argv, expected) in [
            (
                args(&["--fail_on=nope"]),
                "unknown fail_on \"nope\", want info|warning|error",
            ),
            (
                args(&["--fail_on="]),
                "unknown fail_on \"\", want info|warning|error",
            ),
            (
                args(&["--fail_on", "--output=x"]),
                "unknown fail_on \"--output=x\", want info|warning|error",
            ),
        ] {
            assert_eq!(
                run_from(&argv).expect_err("rejected threshold"),
                expected,
                "argv: {argv:?}"
            );
        }
    }

    #[test]
    fn a_repeated_flag_keeps_its_missing_value_message() {
        assert_eq!(
            run_from(&args(&["--output=x", "--output"])).expect_err("repeated flag"),
            "missing value for --output"
        );
    }

    #[test]
    fn parse_error_keeps_claps_own_line_for_kinds_it_does_not_own() {
        for kind in [
            ErrorKind::TooManyValues,
            ErrorKind::MissingRequiredArgument,
            ErrorKind::NoEquals,
        ] {
            let error = clap::Error::raw(kind, "usage: quality_evaluator\n\nsecond line");
            assert_eq!(
                parse_error(error, &args(&[])),
                "error: usage: quality_evaluator",
                "kind: {kind:?}"
            );
        }
    }

    #[test]
    fn parse_error_keeps_claps_own_line_when_the_threshold_would_parse() {
        #[derive(Parser, Debug)]
        #[command(name = "quality_evaluator", disable_help_flag = true)]
        struct Plain {
            #[arg(long = "fail_on", value_parser = reject_loudly)]
            fail_on: Option<String>,
        }

        fn reject_loudly(raw: &str) -> Result<String, String> {
            Err(format!("error: bad value\n{raw}"))
        }

        let error =
            Plain::try_parse_from(["quality_evaluator", "--fail_on=error"]).expect_err("rejected");
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
        assert_eq!(
            parse_error(error, &args(&["--fail_on=error"])),
            "error: invalid value 'error' for '--fail_on <FAIL_ON>': error: bad value"
        );
    }

    #[test]
    fn parse_threshold_failure_is_the_only_threshold_message() {
        assert_eq!(parse_fail_on("error").map(Threshold::name), Ok("error"));
        assert_eq!(
            parse_fail_on("never").expect_err("unknown threshold"),
            "unknown fail_on \"never\", want info|warning|error"
        );
    }
}
