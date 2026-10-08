use super::super::{assert_usage, ArgsError, Command, OperationMode, ReportRequest};
use super::parse;
use crate::test_support::strings;
use dx_output::{OutputMode, Threshold};

#[test]
fn bare_command_parses_with_defaults() {
    let got = parse(&strings(&["lint"])).expect("parse");
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
    let got = parse(&strings(&["coverage", "--min-coverage", "80"])).expect("parse");
    assert_eq!(got.command, Command::Coverage);
    assert_eq!(got.min_coverage, Some(80));
    let inline = parse(&strings(&["coverage", "--min-coverage=100"])).expect("parse");
    assert_eq!(inline.min_coverage, Some(100));
    let zero = parse(&strings(&["coverage", "--min-coverage=0"])).expect("parse");
    assert_eq!(zero.min_coverage, Some(0));
    let bare = parse(&strings(&["coverage"])).expect("parse");
    assert_eq!(bare.min_coverage, None);
}

#[test]
fn min_coverage_rejects_bad_values_and_other_commands() {
    assert_usage(
        &["coverage", "--min-coverage=eighty"],
        parse(&strings(&["coverage", "--min-coverage=eighty"])).unwrap_err(),
        &["min-coverage"],
    );
    assert_usage(
        &["coverage", "--min-coverage=101"],
        parse(&strings(&["coverage", "--min-coverage=101"])).unwrap_err(),
        &["min-coverage"],
    );
    assert_usage(
        &["coverage", "--min-coverage"],
        parse(&strings(&["coverage", "--min-coverage"])).unwrap_err(),
        &["min-coverage"],
    );
    assert_eq!(
        parse(&strings(&["test", "--min-coverage=80"])),
        Err(ArgsError::UnsupportedOption {
            command: "test",
            option: "--min-coverage".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["lint", "--min-coverage=80"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--min-coverage".to_owned(),
        })
    );
}

#[test]
fn check_selects_check_mode() {
    let got = parse(&strings(&["format", "--check"])).expect("parse");
    assert_eq!(got.command, Command::Format);
    assert!(got.check);
    assert_eq!(got.operation(), OperationMode::Check);
    assert_eq!(got.mode(), "check");
}

/// The words one command needs before `--apply`, mirroring the required slots.
fn plain_words(command: Command) -> Vec<String> {
    let mut words = vec![command.name().to_owned()];
    match command {
        Command::Bump => words.extend(strings(&["cargo:demo", "1.0.0"])),
        Command::Migrate | Command::Upgrade => {
            words.extend(strings(&["--from=1.0.0", "--to=2.0.0"]));
        }
        Command::Run | Command::Deploy | Command::Owners | Command::Deps => {
            words.push("//:demo".to_owned());
        }
        Command::Why => words.extend(strings(&["a.rs", "//:demo"])),
        Command::Hooks => words.push("status".to_owned()),
        Command::New => words.push("rust".to_owned()),
        Command::Watch => words.push("build".to_owned()),
        Command::Completion => words.push("bash".to_owned()),
        _ => {}
    }
    words
}

#[test]
fn bare_commands_check_by_default_and_check_spelling_agrees() {
    use clap::ValueEnum;
    for command in Command::value_variants() {
        let command = *command;
        if command == Command::Bazel {
            continue;
        }
        let bare = parse(&plain_words(command)).unwrap_or_else(|error| panic!("parse: {error}"));
        assert!(!bare.apply, "dx {} must not apply unasked", command.name());
        assert_eq!(
            bare.operation(),
            OperationMode::Check,
            "dx {} defaults to check",
            command.name()
        );
    }
    let explicit = parse(&strings(&["lint", "--check"])).expect("explicit check");
    assert!(explicit.check);
    assert_eq!(explicit.operation(), OperationMode::Check);
}

#[test]
fn dry_run_selects_plan_and_stays_orthogonal_to_check() {
    let plan = parse(&strings(&["lint", "--dry-run"])).expect("plan");
    assert!(plan.dry_run);
    assert_eq!(plan.operation(), OperationMode::Plan);
    let both = parse(&strings(&["lint", "--check", "--dry-run"])).expect("orthogonal");
    assert!(both.check);
    assert!(both.dry_run);
    assert_eq!(both.operation(), OperationMode::Plan);
}

#[test]
fn check_and_apply_conflict() {
    for words in [
        vec!["lint", "--check", "--apply"],
        vec!["lint", "--apply", "--check"],
        vec!["fix", "--check", "--apply"],
        vec!["update", "--check", "--apply"],
        vec!["version", "--check", "--apply"],
        vec!["docs", "--check", "--apply"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::ConflictingModes {
                first: "--check",
                second: "--apply",
            }),
            "words: {words:?}"
        );
    }
}

#[test]
fn dry_run_and_apply_conflict() {
    for words in [
        vec!["lint", "--dry-run", "--apply"],
        vec!["lint", "--apply", "--dry-run"],
        vec!["clean", "--dry-run", "--apply"],
        vec!["fix", "--apply", "--dry-run"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::ConflictingModes {
                first: "--dry-run",
                second: "--apply",
            }),
            "words: {words:?}"
        );
    }
    let off = parse(&strings(&["fix", "--dry-run=false", "--apply"])).expect("explicit off");
    assert!(off.apply);
    assert!(!off.dry_run);
    assert_eq!(off.operation(), OperationMode::Apply);
    let repeated = parse(&strings(&["fix", "--apply", "--apply"])).expect("last wins");
    assert!(repeated.apply);
    assert_eq!(repeated.operation(), OperationMode::Apply);
}

#[test]
fn apply_matrix_matches_supports_apply() {
    use clap::ValueEnum;
    for command in Command::value_variants() {
        let command = *command;
        let mut words = plain_words(command);
        words.push("--apply".to_owned());
        if command == Command::Bazel {
            let got = parse(&words).expect("dx bazel forwards every later word");
            assert_eq!(got.bazel_options, strings(&["--apply"]), "words: {words:?}");
            assert_eq!(got.operation(), OperationMode::Check);
            continue;
        }
        if command.supports_apply() {
            let got = parse(&words).unwrap_or_else(|error| {
                panic!("dx {} --apply must parse: {error}", command.name())
            });
            assert!(got.apply, "dx {} --apply sets the flag", command.name());
            assert_eq!(
                got.operation(),
                OperationMode::Apply,
                "dx {} --apply authorizes",
                command.name()
            );
        } else {
            assert_eq!(
                parse(&words),
                Err(ArgsError::UnsupportedOption {
                    command: command.name(),
                    option: "--apply".to_owned(),
                }),
                "dx {} must refuse --apply",
                command.name()
            );
        }
    }
}

#[test]
fn passthrough_apply_does_not_authorize() {
    let got = parse(&strings(&["lint", "--", "--apply"])).expect("passthrough");
    assert!(!got.apply);
    assert_eq!(got.operation(), OperationMode::Check);
    assert_eq!(got.bazel_options, strings(&["--apply"]));
}

#[test]
fn generate_parses_repo_wide_with_bazel_options() {
    let got = parse(&strings(&["generate"])).expect("parse");
    assert_eq!(got.command, Command::Generate);
    assert!(!got.check);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(got.targets.is_empty());
    assert!(got.bazel_options.is_empty());
    assert_eq!(got.mode(), "default");
    let scoped = parse(&strings(&[
        "generate", "--check", "//a:one", "--", "--jobs=4",
    ]))
    .expect("parse");
    assert_eq!(scoped.command, Command::Generate);
    assert!(scoped.check);
    assert_eq!(scoped.targets, strings(&["//a:one"]));
    assert_eq!(scoped.bazel_options, strings(&["--jobs=4"]));
}

#[test]
fn globals_parse_after_the_command() {
    let got = parse(&strings(&[
        "typecheck",
        "--workspace",
        "/repo",
        "--dry-run",
        "--quiet",
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
    let got = parse(&strings(&["lint", "--quiet"])).expect("parse");
    assert_eq!(got.output, OutputMode::Text { quiet: true });
}

#[test]
fn verbose_parses_after_the_command_and_stays_orthogonal_to_quiet() {
    let bare = parse(&strings(&["lint", "--verbose"])).expect("parse");
    assert!(bare.verbose);
    assert!(!bare.quiet);
    let both = parse(&strings(&["lint", "--quiet", "--verbose"])).expect("parse");
    assert!(both.quiet);
    assert!(both.verbose);
    assert_eq!(both.output, OutputMode::Text { quiet: true });
    let short = parse(&strings(&["lint", "-v"])).expect("parse -v");
    assert!(short.verbose);
    let help = match parse(&strings(&["lint", "--verbose", "--help"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("want Help, got {other:?}"),
    };
    assert!(help.contains("--verbose"), "top-level help:\n{help}");
}

#[test]
fn log_level_parses_and_conflicts_with_verbose() {
    use dx_output::LogLevel;
    let got = parse(&strings(&["lint", "--log-level=debug"])).expect("parse");
    assert_eq!(got.log_level, Some(LogLevel::Debug));
    assert!(!got.verbose);
    let spaced = parse(&strings(&["lint", "--log-level", "trace"])).expect("parse spaced");
    assert_eq!(spaced.log_level, Some(LogLevel::Trace));
    for level in ["error", "warn", "info", "debug", "trace"] {
        let got = parse(&strings(&["lint", &format!("--log-level={level}")])).expect("parse level");
        assert_eq!(got.log_level.map(|level| level.name()), Some(level));
    }
    assert_eq!(
        parse(&strings(&["lint", "--log-level=verbose"])),
        Err(ArgsError::BadLogLevel {
            value: "verbose".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["lint", "--log-level=DEBUG"])),
        Err(ArgsError::BadLogLevel {
            value: "DEBUG".to_owned(),
        })
    );
    assert_usage(
        &["lint", "--verbose", "--log-level=debug"],
        parse(&strings(&["lint", "--verbose", "--log-level=debug"])).unwrap_err(),
        &["--log-level"],
    );
    assert_usage(
        &["lint", "-v", "--log-level=info"],
        parse(&strings(&["lint", "-v", "--log-level=info"])).unwrap_err(),
        &["--log-level"],
    );
    assert_usage(
        &["lint", "--log-level"],
        parse(&strings(&["lint", "--log-level"])).unwrap_err(),
        &["--log-level"],
    );
}

#[test]
fn reports_are_repeatable_with_format_shape() {
    let got = parse(&strings(&[
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
    let got = parse(&strings(&["lint", "--", "--jobs=4", "--", "nokeep_going"])).expect("parse");
    assert_eq!(
        got.bazel_options,
        strings(&["--jobs=4", "--", "nokeep_going"])
    );
}

#[test]
fn missing_command_fails() {
    assert_eq!(parse(&strings(&[])), Err(ArgsError::MissingCommand));
    assert_usage(
        &["--quiet"],
        parse(&strings(&["--quiet"])).unwrap_err(),
        &["--quiet"],
    );
}

#[test]
fn unknown_command_fails() {
    let words = ["lintt"];
    assert_usage(
        &words,
        parse(&strings(&words)).unwrap_err(),
        &["lintt", "similar"],
    );
}

#[test]
fn explicit_label_scope_parses_in_order() {
    let got = parse(&strings(&["lint", "//a:one", "@repo//b/...", "//c/..."])).expect("parse");
    assert_eq!(
        got.targets,
        strings(&["//a:one", "@repo//b/...", "//c/..."])
    );
}

#[test]
fn path_scopes_parse_for_resolution() {
    let got = parse(&strings(&[
        "lint",
        "src/main.rs",
        "quality/testdata/",
        "./x.py",
    ]))
    .expect("parse");
    assert_eq!(
        got.targets,
        strings(&["src/main.rs", "quality/testdata/", "./x.py"])
    );
}

#[test]
fn relative_and_empty_scope_fail() {
    assert_eq!(
        parse(&strings(&["lint", ":corpus"])),
        Err(ArgsError::RelativeLabel {
            scope: ":corpus".to_owned(),
        })
    );
    assert_eq!(parse(&strings(&["lint", ""])), Err(ArgsError::EmptyScope));
}

#[test]
fn workflow_commands_parse_scopes_and_options() {
    for command in ["build", "test", "coverage"] {
        let got = parse(&strings(&[
            command, "//a:one", "pkg/a.py", "--", "--jobs=4",
        ]))
        .expect("parse");
        assert_eq!(got.command.name(), command);
        assert_eq!(
            got.targets,
            strings(&["//a:one", "pkg/a.py"]),
            "scopes parse for resolution"
        );
        assert_eq!(got.bazel_options, strings(&["--jobs=4"]));
    }
}

#[test]
fn workflow_commands_reject_quality_only_options() {
    assert_eq!(
        parse(&strings(&["build", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["test", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "test",
            option: "--fail-on".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["coverage", "--check", "--fail-on=info"])),
        Err(ArgsError::UnsupportedOption {
            command: "coverage",
            option: "--check".to_owned(),
        }),
        "check is reported before fail-on"
    );
    assert!(parse(&strings(&["lint", "--check", "--fail-on=error"])).is_ok());
}

#[test]
fn unknown_options_fail() {
    for (words, needle) in [
        (vec!["lint", "--jobs=4"], "--jobs"),
        (vec!["lint", "-q"], "-q"),
        (vec!["lint", "--dry-run=yes"], "--dry-run"),
        (vec!["-"], "-"),
    ] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[needle]);
    }
}

#[test]
fn missing_values_fail() {
    assert_usage(
        &["lint", "--output"],
        parse(&strings(&["lint", "--output"])).unwrap_err(),
        &["--output"],
    );
    assert_usage(
        &["lint", "--workspace", "--quiet", "format"],
        parse(&strings(&["lint", "--workspace", "--quiet", "format"])).unwrap_err(),
        &["--workspace"],
    );
    assert_eq!(
        parse(&strings(&["lint", "--workspace="])),
        Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        })
    );
}

#[test]
fn bad_values_fail() {
    assert_eq!(
        parse(&strings(&["lint", "--output=yaml"])),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["lint", "--fail-on=never"])),
        Err(ArgsError::BadFailOn {
            value: "never".to_owned(),
        })
    );
    for bad in ["sarif", "=out.sarif", "sarif="] {
        let words = ["lint", &format!("--report={bad}")];
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &["report"]);
    }
}

#[test]
fn inline_flag_values_and_empty_workspace_fail() {
    assert_eq!(
        parse(&strings(&["lint", "--workspace", ""])),
        Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        })
    );
    for words in [vec!["lint", "--quiet=x"], vec!["lint", "--check=x"]] {
        let flag = words[1].split('=').next().expect("a flag");
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[flag]);
    }
}

#[test]
fn clean_parses_dry_run_and_bazel() {
    let got = parse(&strings(&["clean"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(!got.bazel_clean);
    assert!(!got.prune_unobserved);
    assert!(!got.dry_run);
    let got = parse(&strings(&["clean", "--dry-run", "--bazel"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(got.dry_run);
    assert!(got.bazel_clean);
    assert!(!got.prune_unobserved);
    let got = parse(&strings(&["clean", "--prune-unobserved"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(got.prune_unobserved);
    assert_eq!(Command::Clean.name(), "clean");
}

#[test]
fn clean_rejects_scopes_and_quality_options() {
    assert_eq!(
        parse(&strings(&["clean", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["clean", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--fail-on".to_owned(),
        })
    );
    let got = parse(&strings(&["clean", "--output=json"])).expect("clean json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&strings(&["clean", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["clean", "--report=sarif=out.sarif"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--report=sarif=out.sarif".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["clean", "//a:one"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "//a:one".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["clean", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--".to_owned(),
        })
    );
    assert_usage(
        &["clean", "--bazel=yes"],
        parse(&strings(&["clean", "--bazel=yes"])).unwrap_err(),
        &["--bazel"],
    );
    assert_eq!(
        parse(&strings(&["lint", "--bazel"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--bazel".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["build", "--bazel"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--bazel".to_owned(),
        })
    );
}

#[test]
fn audit_update_parse_and_reject_unsupported_options() {
    let security = parse(&strings(&["security"])).expect("parse security");
    assert_eq!(security.command, Command::Security);
    assert_eq!(security.command.name(), "security");
    assert!(security.command.is_audit_update());
    assert!(!security.command.is_adoption());
    assert!(!security.command.is_managed());
    assert!(security.targets.is_empty());
    let license = parse(&strings(&["license", "//a:one"])).expect("parse license scope");
    assert_eq!(license.command, Command::License);
    assert_eq!(license.targets, vec!["//a:one".to_owned()]);
    let update = parse(&strings(&["update"])).expect("parse update");
    assert_eq!(update.command, Command::Update);
    assert_eq!(update.command.name(), "update");
    assert!(update.command.is_audit_update());
    let selected = parse(&strings(&["update", "crates"])).expect("parse update selector");
    assert_eq!(selected.targets, vec!["crates".to_owned()]);
    assert_eq!(
        parse(&strings(&["security", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "security",
            option: "--check".to_owned(),
        })
    );
    let check = parse(&strings(&["update", "--check"])).expect("parse update check");
    assert_eq!(check.command, Command::Update);
    assert!(check.check);
    assert_eq!(check.mode(), "check");
    assert_eq!(
        parse(&strings(&["update", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--fail-on".to_owned(),
        })
    );
    let got = parse(&strings(&["update", "--output=json"])).expect("update json");
    assert_eq!(got.command, Command::Update);
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(
        parse(&strings(&["update", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["update", "--report=sarif=out.sarif"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--report=sarif=out.sarif".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["update", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["license", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "license",
            option: "--".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["security", ":target"])),
        Err(ArgsError::RelativeLabel {
            scope: ":target".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["update", ":target"])),
        Err(ArgsError::RelativeLabel {
            scope: ":target".to_owned(),
        })
    );
}
