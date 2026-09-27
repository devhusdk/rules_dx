use super::super::{ArgsError, Command, ReportRequest};
use super::parse;
use dx_output::{OutputMode, Threshold};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

#[test]
fn bare_command_parses_with_defaults() {
    let got = parse(&args(&["lint"])).expect("parse");
    assert_eq!(got.command, Command::Lint);
    assert!(!got.check);
    assert!(!got.debug);
    assert!(!got.release);
    assert_eq!(got.workspace, None);
    assert!(!got.dry_run);
    assert!(!got.quiet);
    assert!(!got.verbose);
    assert_eq!(got.log_level, None);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(got.reports.is_empty());
    assert_eq!(got.fail_on, Threshold::Warning);
    assert!(got.targets.is_empty());
    assert!(got.bazel_options.is_empty());
    assert_eq!(got.mode(), "default");
}

#[test]
fn min_coverage_parses_for_coverage_only() {
    let got = parse(&args(&["coverage", "--min-coverage", "80"])).expect("parse");
    assert_eq!(got.command, Command::Coverage);
    assert_eq!(got.min_coverage, Some(80));
    let inline = parse(&args(&["coverage", "--min-coverage=100"])).expect("parse");
    assert_eq!(inline.min_coverage, Some(100));
    let zero = parse(&args(&["coverage", "--min-coverage=0"])).expect("parse");
    assert_eq!(zero.min_coverage, Some(0));
    let bare = parse(&args(&["coverage"])).expect("parse");
    assert_eq!(bare.min_coverage, None);
}

#[test]
fn min_coverage_rejects_bad_values_and_other_commands() {
    assert_eq!(
        parse(&args(&["coverage", "--min-coverage=eighty"])),
        Err(ArgsError::BadMinCoverage {
            value: "eighty".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["coverage", "--min-coverage=101"])),
        Err(ArgsError::BadMinCoverage {
            value: "101".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["coverage", "--min-coverage"])),
        Err(ArgsError::MissingValue {
            option: "--min-coverage".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["test", "--min-coverage=80"])),
        Err(ArgsError::UnsupportedOption {
            command: "test",
            option: "--min-coverage".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--min-coverage=80"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--min-coverage".to_owned(),
        })
    );
}

#[test]
fn check_selects_check_mode() {
    let got = parse(&args(&["format", "--check"])).expect("parse");
    assert_eq!(got.command, Command::Format);
    assert!(got.check);
    assert_eq!(got.mode(), "check");
}

#[test]
fn generate_parses_repo_wide_with_bazel_options() {
    let got = parse(&args(&["generate"])).expect("parse");
    assert_eq!(got.command, Command::Generate);
    assert!(!got.check);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(got.targets.is_empty());
    assert!(got.bazel_options.is_empty());
    assert_eq!(got.mode(), "default");
    let scoped =
        parse(&args(&["generate", "--check", "//a:one", "--", "--jobs=4"])).expect("parse");
    assert_eq!(scoped.command, Command::Generate);
    assert!(scoped.check);
    assert_eq!(scoped.targets, args(&["//a:one"]));
    assert_eq!(scoped.bazel_options, args(&["--jobs=4"]));
}

#[test]
fn globals_parse_before_and_after_command() {
    let got = parse(&args(&[
        "--workspace",
        "/repo",
        "--dry-run",
        "--quiet",
        "typecheck",
        "--output=json",
        "--fail-on=error",
    ]))
    .expect("parse");
    assert_eq!(got.command, Command::Typecheck);
    assert_eq!(got.workspace, Some("/repo".to_owned()));
    assert!(got.dry_run);
    assert!(got.quiet);
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(got.fail_on, Threshold::Error);
}

#[test]
fn quiet_applies_to_text_output() {
    let got = parse(&args(&["lint", "--quiet"])).expect("parse");
    assert_eq!(got.output, OutputMode::Text { quiet: true });
}

#[test]
fn verbose_parses_before_and_after_command_and_stays_orthogonal_to_quiet() {
    let bare = parse(&args(&["lint", "--verbose"])).expect("parse");
    assert!(bare.verbose);
    assert!(!bare.quiet);
    let before = parse(&args(&["--verbose", "lint"])).expect("parse");
    assert!(before.verbose);
    let both = parse(&args(&["lint", "--quiet", "--verbose"])).expect("parse");
    assert!(both.quiet);
    assert!(both.verbose);
    assert_eq!(both.output, OutputMode::Text { quiet: true });
    let short = parse(&args(&["lint", "-v"])).expect("parse -v");
    assert!(short.verbose);
    let short_before = parse(&args(&["-v", "lint"])).expect("parse -v before");
    assert!(short_before.verbose);
    let help = match parse(&args(&["--verbose", "--help"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("want Help, got {other:?}"),
    };
    assert!(help.contains("--verbose"), "top-level help:\n{help}");
}

#[test]
fn log_level_parses_and_conflicts_with_verbose() {
    use dx_output::LogLevel;
    let got = parse(&args(&["lint", "--log-level=debug"])).expect("parse");
    assert_eq!(got.log_level, Some(LogLevel::Debug));
    assert!(!got.verbose);
    let spaced = parse(&args(&["lint", "--log-level", "trace"])).expect("parse spaced");
    assert_eq!(spaced.log_level, Some(LogLevel::Trace));
    let before = parse(&args(&["--log-level=warn", "lint"])).expect("parse before");
    assert_eq!(before.log_level, Some(LogLevel::Warn));
    for level in ["error", "warn", "info", "debug", "trace"] {
        let got = parse(&args(&["lint", &format!("--log-level={level}")])).expect("parse level");
        assert_eq!(got.log_level.map(|level| level.name()), Some(level));
    }
    assert_eq!(
        parse(&args(&["lint", "--log-level=verbose"])),
        Err(ArgsError::BadLogLevel {
            value: "verbose".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--log-level=DEBUG"])),
        Err(ArgsError::BadLogLevel {
            value: "DEBUG".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--verbose", "--log-level=debug"])),
        Err(ArgsError::ConflictingVerboseLogLevel)
    );
    assert_eq!(
        parse(&args(&["lint", "-v", "--log-level=info"])),
        Err(ArgsError::ConflictingVerboseLogLevel)
    );
    assert_eq!(
        parse(&args(&["lint", "--log-level"])),
        Err(ArgsError::MissingValue {
            option: "--log-level".to_owned(),
        })
    );
}

#[test]
fn reports_are_repeatable_with_format_shape() {
    let got = parse(&args(&[
        "lint",
        "--report",
        "sarif=reports/lint.sarif",
        "--report=sarif=/tmp/extra.sarif",
    ]))
    .expect("parse");
    assert_eq!(
        got.reports,
        vec![
            ReportRequest {
                format: "sarif".to_owned(),
                destination: "reports/lint.sarif".to_owned(),
            },
            ReportRequest {
                format: "sarif".to_owned(),
                destination: "/tmp/extra.sarif".to_owned(),
            },
        ]
    );
}

#[test]
fn bazel_options_forward_verbatim_after_separator() {
    let got = parse(&args(&["lint", "--", "--jobs=4", "--", "nokeep_going"])).expect("parse");
    assert_eq!(got.bazel_options, args(&["--jobs=4", "--", "nokeep_going"]));
}

#[test]
fn missing_command_fails() {
    assert_eq!(parse(&args(&[])), Err(ArgsError::MissingCommand));
    assert_eq!(parse(&args(&["--quiet"])), Err(ArgsError::MissingCommand));
}

#[test]
fn unknown_command_fails() {
    assert_eq!(
        parse(&args(&["bogus"])),
        Err(ArgsError::UnknownCommand {
            command: "bogus".to_owned(),
            suggestion: None,
        })
    );
}

#[test]
fn explicit_label_scope_parses_in_order() {
    let got = parse(&args(&["lint", "//a:one", "@repo//b/...", "//c/..."])).expect("parse");
    assert_eq!(got.targets, args(&["//a:one", "@repo//b/...", "//c/..."]));
}

#[test]
fn path_scopes_parse_for_resolution() {
    let got = parse(&args(&[
        "lint",
        "src/main.rs",
        "quality/testdata/",
        "./x.py",
    ]))
    .expect("parse");
    assert_eq!(
        got.targets,
        args(&["src/main.rs", "quality/testdata/", "./x.py"])
    );
}

#[test]
fn relative_and_empty_scope_fail() {
    assert_eq!(
        parse(&args(&["lint", ":corpus"])),
        Err(ArgsError::RelativeLabel {
            scope: ":corpus".to_owned(),
        })
    );
    assert_eq!(parse(&args(&["lint", ""])), Err(ArgsError::EmptyScope));
}

#[test]
fn workflow_commands_parse_scopes_and_options() {
    for command in ["build", "test", "coverage"] {
        let got = parse(&args(&[command, "//a:one", "pkg/a.py", "--", "--jobs=4"])).expect("parse");
        assert_eq!(got.command.name(), command);
        assert_eq!(
            got.targets,
            args(&["//a:one", "pkg/a.py"]),
            "scopes parse for resolution"
        );
        assert_eq!(got.bazel_options, args(&["--jobs=4"]));
    }
}

#[test]
fn workflow_commands_reject_quality_only_options() {
    assert_eq!(
        parse(&args(&["build", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["test", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "test",
            option: "--fail-on".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["coverage", "--check", "--fail-on=info"])),
        Err(ArgsError::UnsupportedOption {
            command: "coverage",
            option: "--check".to_owned(),
        }),
        "check is reported before fail-on"
    );
    assert!(parse(&args(&["lint", "--check", "--fail-on=error"])).is_ok());
}

#[test]
fn unknown_options_fail() {
    assert_eq!(
        parse(&args(&["lint", "--jobs=4"])),
        Err(ArgsError::UnknownOption {
            option: "--jobs=4".to_owned(),
            suggestion: None,
        })
    );
    assert_eq!(
        parse(&args(&["lint", "-q"])),
        Err(ArgsError::UnknownOption {
            option: "-q".to_owned(),
            suggestion: None,
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--dry-run=yes"])),
        Err(ArgsError::UnknownOption {
            option: "--dry-run=yes".to_owned(),
            suggestion: None,
        })
    );
    assert_eq!(
        parse(&args(&["-"])),
        Err(ArgsError::UnknownOption {
            option: "-".to_owned(),
            suggestion: None,
        })
    );
}

#[test]
fn missing_values_fail() {
    assert_eq!(
        parse(&args(&["lint", "--output"])),
        Err(ArgsError::MissingValue {
            option: "--output".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--workspace", "--quiet", "format"])),
        Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--workspace="])),
        Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        })
    );
}

#[test]
fn bad_values_fail() {
    assert_eq!(
        parse(&args(&["lint", "--output=yaml"])),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--fail-on=never"])),
        Err(ArgsError::BadFailOn {
            value: "never".to_owned(),
        })
    );
    for bad in ["sarif", "=out.sarif", "sarif=", ""] {
        assert_eq!(
            parse(&args(&["lint", &format!("--report={bad}")])),
            Err(ArgsError::BadReport {
                value: bad.to_owned(),
            })
        );
    }
}

#[test]
fn inline_flag_values_and_empty_workspace_fail() {
    assert_eq!(
        parse(&args(&["lint", "--workspace", ""])),
        Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--quiet=x"])),
        Err(ArgsError::UnknownOption {
            option: "--quiet=x".to_owned(),
            suggestion: None,
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--check=x"])),
        Err(ArgsError::UnknownOption {
            option: "--check=x".to_owned(),
            suggestion: None,
        })
    );
}

#[test]
fn clean_parses_dry_run_and_bazel() {
    let got = parse(&args(&["clean"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(!got.bazel_clean);
    assert!(!got.dry_run);
    let got = parse(&args(&["clean", "--dry-run", "--bazel"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(got.dry_run);
    assert!(got.bazel_clean);
    assert_eq!(Command::Clean.name(), "clean");
    assert!(!Command::Clean.is_workflow());
    assert!(!Command::Clean.is_umbrella());
}

#[test]
fn clean_rejects_scopes_and_quality_options() {
    assert_eq!(
        parse(&args(&["clean", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["clean", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--fail-on".to_owned(),
        })
    );
    let got = parse(&args(&["clean", "--output=json"])).expect("clean json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&args(&["clean", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["clean", "--report=sarif=out.sarif"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--report=sarif=out.sarif".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["clean", "//a:one"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "//a:one".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["clean", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["clean", "--bazel=yes"])),
        Err(ArgsError::UnknownOption {
            option: "--bazel=yes".to_owned(),
            suggestion: None,
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--bazel"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--bazel".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["build", "--bazel"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--bazel".to_owned(),
        })
    );
}

#[test]
fn audit_update_parse_and_reject_unsupported_options() {
    let security = parse(&args(&["security"])).expect("parse security");
    assert_eq!(security.command, Command::Security);
    assert_eq!(security.command.name(), "security");
    assert!(security.command.is_audit_update());
    assert!(!security.command.is_workflow());
    assert!(!security.command.is_adoption());
    assert!(!security.command.is_managed());
    assert!(security.targets.is_empty());
    let license = parse(&args(&["license", "//a:one"])).expect("parse license scope");
    assert_eq!(license.command, Command::License);
    assert_eq!(license.targets, vec!["//a:one".to_owned()]);
    let update = parse(&args(&["update"])).expect("parse update");
    assert_eq!(update.command, Command::Update);
    assert_eq!(update.command.name(), "update");
    assert!(update.command.is_audit_update());
    let selected = parse(&args(&["update", "crates"])).expect("parse update selector");
    assert_eq!(selected.targets, vec!["crates".to_owned()]);
    assert_eq!(
        parse(&args(&["security", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "security",
            option: "--check".to_owned(),
        })
    );
    let check = parse(&args(&["update", "--check"])).expect("parse update check");
    assert_eq!(check.command, Command::Update);
    assert!(check.check);
    assert_eq!(check.mode(), "check");
    assert_eq!(
        parse(&args(&["update", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--fail-on".to_owned(),
        })
    );
    let got = parse(&args(&["update", "--output=json"])).expect("update json");
    assert_eq!(got.command, Command::Update);
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(
        parse(&args(&["update", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["update", "--report=sarif=out.sarif"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--report=sarif=out.sarif".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["update", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["license", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "license",
            option: "--".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["security", ":target"])),
        Err(ArgsError::RelativeLabel {
            scope: ":target".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["update", ":target"])),
        Err(ArgsError::RelativeLabel {
            scope: ":target".to_owned(),
        })
    );
}
