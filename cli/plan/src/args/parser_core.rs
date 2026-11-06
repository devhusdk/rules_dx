use super::super::{ArgsError, Command, OperationMode, ReportRequest, assert_usage};
use super::parse;
use dx_output::{OutputMode, Threshold};
use dx_testing::cli::strings;

#[test]
fn bare_command_parses_with_defaults() {
    let got = parse(&strings(&["lint"])).expect("parse");
    assert_eq!(got.command, Command::Lint);
    assert!(!got.debug);
    assert!(!got.release);
    assert_eq!(got.workspace, None);
    assert!(!got.quiet);
    assert!(!got.verbose);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(got.reports.is_empty());
    assert_eq!(got.fail_on, Threshold::Warning);
    assert!(got.targets.is_empty());
    assert!(got.bazel_options.is_empty());
    assert!(got.bazel_startup_options.is_empty());
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
fn min_changed_line_coverage_parses_for_coverage_only() {
    let got = parse(&strings(&["coverage", "--min-changed-line-coverage=80"])).expect("parse");
    assert_eq!(got.min_changed_line_coverage, Some(80));
    assert_eq!(got.min_coverage, None);
    let spaced = parse(&strings(&[
        "coverage",
        "--min-changed-line-coverage",
        "100",
    ]))
    .expect("parse");
    assert_eq!(spaced.min_changed_line_coverage, Some(100));
    let zero = parse(&strings(&["coverage", "--min-changed-line-coverage=0"])).expect("parse");
    assert_eq!(zero.min_changed_line_coverage, Some(0));
    let bare = parse(&strings(&["coverage"])).expect("parse");
    assert_eq!(bare.min_changed_line_coverage, None);
    for bad in ["101", "-1", "eighty"] {
        let words = [
            "coverage".to_owned(),
            format!("--min-changed-line-coverage={bad}"),
        ];
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        assert_usage(
            &refs,
            parse(&words).unwrap_err(),
            &["min-changed-line-coverage"],
        );
    }
    assert_eq!(
        parse(&strings(&["test", "--min-changed-line-coverage=80"])),
        Err(ArgsError::UnsupportedOption {
            command: "test",
            option: "--min-changed-line-coverage".to_owned(),
        })
    );
}

#[test]
fn min_coverage_rejects_bad_values_and_other_commands() {
    assert_usage(
        ["coverage", "--min-coverage=eighty"],
        parse(&strings(&["coverage", "--min-coverage=eighty"])).unwrap_err(),
        &["min-coverage"],
    );
    assert_usage(
        ["coverage", "--min-coverage=101"],
        parse(&strings(&["coverage", "--min-coverage=101"])).unwrap_err(),
        &["min-coverage"],
    );
    assert_usage(
        ["coverage", "--min-coverage"],
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

/// The words one command needs before `--apply`, mirroring the required slots.
fn plain_words(command: Command) -> Vec<String> {
    let mut words = vec![command.name().to_owned()];
    match command {
        Command::Upgrade => {
            words.extend(strings(&["--to=2.0.0"]));
        }
        Command::Run | Command::Deploy => {
            words.push("//:demo".to_owned());
        }
        Command::Watch => words.extend(strings(&["run", "//:demo"])),
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
        let bare = parse(&plain_words(command)).unwrap_or_else(|error| panic!("parse: {error}"));
        assert!(!bare.apply, "dx {} must not apply unasked", command.name());
        let want = if command == Command::Sync {
            OperationMode::Apply
        } else {
            OperationMode::Check
        };
        assert_eq!(bare.operation(), want, "dx {}", command.name());
    }
}

#[test]
fn removed_dry_run_flag_is_unknown() {
    for words in [
        vec!["lint", "--dry-run"],
        vec!["lint", "--dry-run=true"],
        vec!["lint", "--dry-run=false"],
        vec!["clean", "--dry-run", "--apply"],
        vec!["check", "--apply", "--dry-run"],
    ] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &["--dry-run"]);
    }
}

#[test]
fn repeated_apply_is_idempotent() {
    let repeated = parse(&strings(&["check", "--apply", "--apply"])).expect("last wins");
    assert!(repeated.apply);
    assert_eq!(repeated.operation(), OperationMode::Apply);
}

#[test]
fn apply_is_accepted_exactly_by_the_apply_commands() {
    use super::super::command::owners::APPLY_COMMANDS;
    use clap::ValueEnum;
    for command in Command::value_variants() {
        let command = *command;
        let mut words = plain_words(command);
        if command == Command::Watch {
            words[1] = "check".to_owned();
        }
        words.push("--apply".to_owned());
        let listed = APPLY_COMMANDS.contains(&command);
        assert_eq!(command.supports_apply(), listed, "dx {}", command.name());
        if listed {
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
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(got.targets.is_empty());
    assert!(got.bazel_options.is_empty());
    let scoped = parse(&strings(&["generate", "//a:one", "--", "--jobs=4"])).expect("parse");
    assert_eq!(scoped.command, Command::Generate);
    assert_eq!(scoped.targets, strings(&["//a:one"]));
    assert_eq!(scoped.bazel_options, strings(&["--jobs=4"]));
}

#[test]
fn globals_parse_after_the_command() {
    let got = parse(&strings(&[
        "typecheck",
        "--workspace",
        "/repo",
        "--quiet",
        "--output=json",
        "--fail-on=error",
    ]))
    .expect("parse");
    assert_eq!(got.command, Command::Typecheck);
    assert_eq!(got.workspace, Some("/repo".to_owned()));
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
fn short_options_match_their_long_forms() {
    let long = parse(&strings(&["lint", "--quiet", "--verbose", "--output=json"])).expect("long");
    for words in [
        vec!["lint", "-q", "-v", "-o", "json"],
        vec!["lint", "-q", "-v", "-ojson"],
        vec!["lint", "-qv", "-o=json"],
        vec!["lint", "-vq", "--output", "json"],
    ] {
        let got = parse(&strings(&words)).unwrap_or_else(|error| panic!("{words:?}: {error:?}"));
        assert!(got.quiet && got.verbose, "{words:?}");
        assert_eq!(got.output, long.output, "{words:?}");
    }
    let repeated = parse(&strings(&["lint", "-v", "-v"])).expect("-v -v");
    assert!(repeated.verbose);
    let sub = parse(&strings(&["lint", "-q=false"])).expect("-q=false");
    assert!(!sub.quiet);
}

#[test]
fn short_booleans_leave_the_target_alone() {
    let got = parse(&strings(&["build", "-q", "//app:bin"])).expect("parse");
    assert!(got.quiet);
    assert_eq!(got.targets, strings(&["//app:bin"]));
}

#[test]
fn short_options_after_the_separator_belong_to_the_payload() {
    let got = parse(&strings(&["run", "//app:bin", "--", "-v", "-q", "-o", "x"])).expect("parse");
    assert!(!got.verbose && !got.quiet);
    assert_eq!(got.app_args(), strings(&["-v", "-q", "-o", "x"]));
    assert!(got.bazel_options.is_empty());
}

#[test]
fn short_output_rejects_missing_and_invalid_values() {
    assert!(parse(&strings(&["lint", "-o"])).is_err());
    assert!(parse(&strings(&["lint", "-o", "nope"])).is_err());
}

#[test]
fn log_level_is_rejected() {
    for words in [
        vec!["lint", "--log-level=debug"],
        vec!["lint", "--log-level", "trace"],
        vec!["lint", "-v", "--log-level=info"],
        vec!["lint", "--log-level"],
    ] {
        assert_usage(
            &words,
            parse(&strings(&words)).unwrap_err(),
            &["--log-level"],
        );
    }
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
        ["--quiet"],
        parse(&strings(&["--quiet"])).unwrap_err(),
        &["--quiet"],
    );
}

#[test]
fn unknown_command_fails() {
    let words = ["lintt"];
    assert_usage(
        words,
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
        parse(&strings(&["test", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "test",
            option: "--fail-on".to_owned(),
        })
    );
    assert!(parse(&strings(&["lint", "--fail-on=error"])).is_ok());
}

#[test]
fn unknown_options_fail() {
    for (words, needle) in [
        (vec!["lint", "--parallel=4"], "--parallel"),
        (vec!["lint", "-z"], "-z"),
        (vec!["lint", "--dry-run"], "--dry-run"),
        (vec!["-"], "-"),
    ] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[needle]);
    }
}

#[test]
fn missing_values_fail() {
    assert_usage(
        ["lint", "--output"],
        parse(&strings(&["lint", "--output"])).unwrap_err(),
        &["--output"],
    );
    assert_usage(
        ["lint", "--workspace", "--quiet", "format"],
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
        assert_usage(words, parse(&strings(&words)).unwrap_err(), &["report"]);
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
    {
        let words = vec!["lint", "--quiet=x"];
        let flag = words[1].split('=').next().expect("a flag");
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[flag]);
    }
}

#[test]
fn clean_parses_prune_unobserved() {
    let got = parse(&strings(&["clean"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(!got.prune_unobserved);
    let got = parse(&strings(&["clean", "--prune-unobserved"])).expect("parse");
    assert_eq!(got.command, Command::Clean);
    assert!(got.prune_unobserved);
    assert_eq!(Command::Clean.name(), "clean");
}

#[test]
fn clean_rejects_scopes_and_quality_options() {
    let got = parse(&strings(&["clean", "--apply"])).expect("clean apply parses");
    assert!(got.applies());
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
        parse(&strings(&["clean", "--diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--diff".to_owned(),
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
        ["clean", "--bazel=yes"],
        parse(&strings(&["clean", "--bazel=yes"])).unwrap_err(),
        &["--bazel"],
    );
}

#[test]
fn audit_update_parse_and_reject_unsupported_options() {
    let security = parse(&strings(&["security"])).expect("parse security");
    assert_eq!(security.command, Command::Security);
    assert_eq!(security.command.name(), "security");
    assert!(!security.command.is_adoption());
    assert!(!security.command.is_managed());
    assert!(security.targets.is_empty());
    let license = parse(&strings(&["license", "//a:one"])).expect("parse license scope");
    assert_eq!(license.command, Command::License);
    assert_eq!(license.targets, vec!["//a:one".to_owned()]);
    let update = parse(&strings(&["update"])).expect("parse update");
    assert_eq!(update.command, Command::Update);
    assert_eq!(update.command.name(), "update");
    let selected = parse(&strings(&["update", "crates"])).expect("parse update selector");
    assert_eq!(selected.targets, vec!["crates".to_owned()]);
    assert!(
        parse(&strings(&["update", "--outdated", "--fail-on=unused"]))
            .expect("update fail-on")
            .fail_on_unused
    );
    assert!(parse(&strings(&["lint", "--fail-on=unused"])).is_err());
    let got = parse(&strings(&["update", "--output=json"])).expect("update json");
    assert_eq!(got.command, Command::Update);
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(
        parse(&strings(&["update", "--diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "update",
            option: "--diff".to_owned(),
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

#[test]
fn every_command_without_a_passthrough_refuses_it_the_same_way() {
    use clap::ValueEnum;
    let refusing: Vec<Command> = Command::value_variants()
        .iter()
        .copied()
        .filter(|command| !super::super::grammar::advertises_passthrough(*command))
        .collect();
    assert!(refusing.contains(&Command::Clean));
    assert!(refusing.contains(&Command::Worktree));
    for command in refusing {
        assert_eq!(
            parse(&strings(&[command.name(), "--", "--jobs=4"])),
            Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            }),
            "{}",
            command.name()
        );
    }
}

#[test]
fn startup_options_parse_repeatably_and_stay_ordered() {
    let got = parse(&strings(&[
        "build",
        "--bazel-startup-option=--output_base=/tmp/a",
        "--bazel-startup-option=--output_user_root=/tmp/b",
        "//...",
    ]))
    .expect("parse");
    assert_eq!(
        got.bazel_startup_options,
        vec![
            "--output_base=/tmp/a".to_owned(),
            "--output_user_root=/tmp/b".to_owned(),
        ]
    );
    let build = parse(&strings(&[
        "build",
        "--bazel-startup-option=--output_base=/tmp/a",
        "//:demo",
    ]))
    .expect("build takes the global flag");
    assert_eq!(
        build.bazel_startup_options,
        vec!["--output_base=/tmp/a".to_owned()]
    );
}

#[test]
fn startup_options_accept_a_separate_dashed_token() {
    let got = parse(&strings(&[
        "build",
        "--bazel-startup-option",
        "--output_base=/tmp/a",
        "--bazel-startup-option=--output_user_root=/tmp/b",
        "--bazel-startup-option",
        "--output_base=/tmp/c",
        "//...",
    ]))
    .expect("parse");
    assert_eq!(
        got.bazel_startup_options,
        vec![
            "--output_base=/tmp/a".to_owned(),
            "--output_user_root=/tmp/b".to_owned(),
            "--output_base=/tmp/c".to_owned(),
        ]
    );
    assert_eq!(got.targets, vec!["//...".to_owned()]);
    for swallowed in ["--quiet", "--"] {
        let err = parse(&strings(&[
            "build",
            "--bazel-startup-option",
            swallowed,
            "//...",
        ]))
        .expect_err("a separate non-startup word is taken as the token");
        assert!(
            matches!(err, ArgsError::BadStartupOption { .. }),
            "{swallowed} produced {err:?}"
        );
        assert!(
            err.to_string().contains("--output_base"),
            "{swallowed}: {err}"
        );
    }
    let words = ["build", "--bazel-startup-option"];
    assert_usage(
        words,
        parse(&strings(&words)).unwrap_err(),
        &["--bazel-startup-option"],
    );
}

#[test]
fn startup_options_keep_spaces_and_unicode_in_one_token() {
    let got = parse(&strings(&[
        "test",
        "--bazel-startup-option=--output_base=/tmp/a b/ünïcode",
        "//...",
    ]))
    .expect("parse");
    assert_eq!(
        got.bazel_startup_options,
        vec!["--output_base=/tmp/a b/ünïcode".to_owned()]
    );
}

#[test]
fn startup_options_reject_missing_and_unqualified_tokens() {
    for words in [
        vec!["build", "--bazel-startup-option=--jobs=4"],
        vec!["build", "--bazel-startup-option=--home_rc"],
        vec!["build", "--bazel-startup-option=--nohome_rc"],
        vec!["build", "--bazel-startup-option=--bazelrc=/tmp/rc"],
        vec!["build", "--bazel-startup-option=--bogus=1"],
        vec!["build", "--bazel-startup-option=plain"],
    ] {
        let err = parse(&strings(&words)).expect_err("unqualified must fail");
        assert!(
            matches!(err, ArgsError::BadStartupOption { .. }),
            "{words:?} produced {err:?}"
        );
        assert!(
            err.to_string().contains("--output_base"),
            "{words:?}: {err}"
        );
    }
    for (words, name) in [
        (
            vec!["build", "--bazel-startup-option=--output_base"],
            "--output_base",
        ),
        (
            vec!["build", "--bazel-startup-option=--output_base="],
            "--output_base",
        ),
        (
            vec!["test", "--bazel-startup-option=--output_user_root"],
            "--output_user_root",
        ),
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::BadStartupOption {
                value: name.to_owned(),
            }),
            "{words:?} names its missing value"
        );
    }
}

#[test]
fn fail_on_uncovered_is_a_security_option() {
    let got = parse(&strings(&["security", "--fail-on=uncovered"])).expect("security parses");
    assert!(got.fail_on_uncovered);
    assert_eq!(got.fail_on, Threshold::Warning);
    assert!(
        !parse(&strings(&["security"]))
            .expect("plain")
            .fail_on_uncovered
    );
    assert!(parse(&strings(&["license", "--fail-on=uncovered"])).is_err());
    assert!(parse(&strings(&["lint", "--fail-on=uncovered"])).is_err());
}

#[test]
fn accept_needs_a_reason_and_an_expiry() {
    for command in ["security", "license"] {
        let got = parse(&strings(&[
            command,
            "--accept",
            "GHSA-1",
            "--accept=GHSA-2:pkg",
            "--reason",
            "reviewed",
            "--expires",
            "2999-01-01",
        ]))
        .expect("accept parses");
        assert_eq!(got.accept, ["GHSA-1", "GHSA-2:pkg"]);
        assert_eq!(got.reason.as_deref(), Some("reviewed"));
        assert_eq!(got.expires.as_deref(), Some("2999-01-01"));
        for words in [
            vec![command, "--accept", "x"],
            vec![command, "--accept", "x", "--reason", "r"],
            vec![command, "--accept", "x", "--expires", "2999-01-01"],
        ] {
            assert!(
                matches!(parse(&strings(&words)), Err(ArgsError::MissingValue { .. })),
                "{words:?}"
            );
        }
        assert!(parse(&strings(&[command, "--reason", "r"])).is_err());
        assert!(parse(&strings(&[command, "--expires", "2999-01-01"])).is_err());
    }
    assert!(parse(&strings(&["lint", "--accept", "x"])).is_err());
}

#[test]
fn fail_on_no_collector_is_a_coverage_option() {
    let got = parse(&strings(&["coverage", "--fail-on=no-collector"])).expect("coverage parses");
    assert!(got.fail_on_no_collector);
    assert!(
        !parse(&strings(&["coverage"]))
            .expect("plain")
            .fail_on_no_collector
    );
    assert!(parse(&strings(&["test", "--fail-on=no-collector"])).is_err());
    assert!(parse(&strings(&["lint", "--fail-on=no-collector"])).is_err());
}

#[test]
fn lint_online_sets_the_lychee_build_flag() {
    let got = parse(&strings(&["lint", "--online"])).expect("parse");
    assert_eq!(got.bazel_options[0], "--//config:lint_online=true");
    let plain = parse(&strings(&["lint"])).expect("parse");
    assert!(plain.bazel_options.is_empty());
}

#[test]
fn unused_is_not_a_command_and_none_is_not_a_fail_on_value() {
    assert!(parse(&strings(&["unused"])).is_err());
    for other in ["lint", "check", "update", "security"] {
        assert!(
            parse(&strings(&[other, "--fail-on=none"])).is_err(),
            "{other} must reject --fail-on=none"
        );
    }
}
