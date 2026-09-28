use super::super::{ArgsError, Command};
use super::parse;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

#[test]
fn strict_unknown_options_fail_with_whole_token() {
    for words in [
        vec!["lint", "--jobs=4"],
        vec!["lint", "-q"],
        vec!["lint", "--dry-run=yes"],
        vec!["lint", "--quiet=x"],
        vec!["lint", "--check=x"],
        vec!["clean", "--bazel=yes"],
        vec!["init", "--force"],
        vec!["hooks", "install", "--force"],
        vec!["-"],
        vec!["lint", "--bogus"],
        vec!["lint", "--bogus=1"],
    ] {
        match parse(&args(&words)) {
            Err(ArgsError::UnknownOption { option, .. }) => {
                assert!(
                    words.contains(&option.as_str()),
                    "words: {words:?} echoed as {option:?}"
                );
            }
            other => panic!("words: {words:?}: want UnknownOption, got {other:?}"),
        }
    }
    assert_eq!(
        parse(&args(&["lint", "--jobs=4"])),
        Err(ArgsError::UnknownOption {
            option: "--jobs=4".to_owned(),
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
}

#[test]
fn strict_missing_values_fail_with_bare_flag() {
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
    assert_eq!(
        parse(&args(&["coverage", "--min-coverage"])),
        Err(ArgsError::MissingValue {
            option: "--min-coverage".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["docs", "--port"])),
        Err(ArgsError::MissingValue {
            option: "--port".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["docs", "--host"])),
        Err(ArgsError::MissingValue {
            option: "--host".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--color"])),
        Err(ArgsError::MissingValue {
            option: "--color".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["migrate", "--from", "--to=2.0.0"])),
        Err(ArgsError::MissingValue {
            option: "--from".to_owned(),
        })
    );
}

#[test]
fn strict_hyphen_values_are_never_consumed_as_option_values() {
    assert_eq!(
        parse(&args(&["lint", "--output", "--quiet"])),
        Err(ArgsError::MissingValue {
            option: "--output".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--workspace", "--quiet"])),
        Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["build", "--output", "--dry-run"])),
        Err(ArgsError::MissingValue {
            option: "--output".to_owned(),
        })
    );
}

#[test]
fn strict_bad_values_fail_with_contract_shapes() {
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
        parse(&args(&["lint", "--color=bright"])),
        Err(ArgsError::BadColor {
            value: "bright".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["docs", "--serve", "--port=0"])),
        Err(ArgsError::MissingValue {
            option: "--port".to_owned(),
        })
    );
}

#[test]
fn strict_typo_suggestions_come_from_the_same_grammar() {
    assert_eq!(
        parse(&args(&["lintt"])),
        Err(ArgsError::UnknownCommand {
            command: "lintt".to_owned(),
            suggestion: Some("lint".to_owned()),
        })
    );
    assert_eq!(
        parse(&args(&["--ouptut=json"])),
        Err(ArgsError::UnknownOption {
            option: "--ouptut=json".to_owned(),
            suggestion: Some("--output".to_owned()),
        })
    );
}

#[test]
fn strict_repeated_flags_are_last_wins() {
    for (words, command) in [
        (vec!["build", "--here", "--cwd"], Command::Build),
        (vec!["build", "--cwd", "--here"], Command::Build),
        (vec!["lint", "--check", "--check"], Command::Lint),
        (vec!["lint", "--quiet", "--quiet"], Command::Lint),
        (vec!["build", "-vv"], Command::Build),
        (vec!["update", "--offline", "--frozen"], Command::Update),
        (vec!["clean", "--bazel", "--bazel"], Command::Clean),
    ] {
        let got = parse(&args(&words)).unwrap_or_else(|error| panic!("words: {words:?}: {error}"));
        assert_eq!(got.command, command, "words: {words:?}");
    }
    assert!(
        parse(&args(&["build", "--here", "--cwd"]))
            .expect("here")
            .here
    );
    assert!(parse(&args(&["build", "-vv"])).expect("verbose").verbose);
    assert!(
        parse(&args(&["build", "--debug", "--release"]))
            .is_err_and(|error| matches!(error, ArgsError::ConflictingProfiles)),
        "repeatable flags must not weaken the profile conflict"
    );
}

#[test]
fn strict_bazel_tail_forwards_verbatim() {
    let got = parse(&args(&["bazel", "build", "//...", "--", "--jobs=4"])).expect("parse");
    assert_eq!(got.command, Command::Bazel);
    assert_eq!(
        got.bazel_options,
        vec![
            "build".to_owned(),
            "//...".to_owned(),
            "--jobs=4".to_owned()
        ]
    );
    for words in [
        vec!["bazel", "--jobs=4"],
        vec!["bazel", "--check"],
        vec!["bazel", "--pin=0.1.0"],
        vec!["bazel", "build", "--here"],
    ] {
        let got = parse(&args(&words)).expect("verbatim");
        assert_eq!(got.command, Command::Bazel, "words: {words:?}");
    }
    for words in [
        vec!["--output=json", "bazel", "version"],
        vec!["--check", "bazel", "version"],
        vec!["--here", "bazel", "version"],
    ] {
        assert!(
            matches!(
                parse(&args(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?}"
        );
    }
}

#[test]
fn strict_help_verb_redirects_to_generated_help() {
    for words in [vec!["Help"], vec!["HELP"]] {
        match parse(&args(&words)) {
            Err(ArgsError::UnknownCommand { command, .. }) => {
                assert_eq!(command, words[0]);
            }
            other => panic!("words: {words:?}: want UnknownCommand, got {other:?}"),
        }
    }
    for words in [vec!["help"], vec!["help", "lint"], vec!["help", "status"]] {
        match parse(&args(&words)) {
            Err(ArgsError::Help { text }) => {
                if words.len() > 1 {
                    assert!(
                        text.contains(words[1]),
                        "words: {words:?}: help missing command"
                    );
                } else {
                    assert!(text.contains("Commands:"), "words: {words:?}");
                }
            }
            other => panic!("words: {words:?}: want Help, got {other:?}"),
        }
    }
    let verb = match parse(&args(&["help", "lint"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("help lint: want Help, got {other:?}"),
    };
    let flag = match parse(&args(&["lint", "--help"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("lint --help: want Help, got {other:?}"),
    };
    assert_eq!(verb, flag, "help verb must redirect to per-command help");
    for flag in ["--help", "-h"] {
        match parse(&args(&[flag])) {
            Err(ArgsError::Help { .. }) => {}
            other => panic!("{flag}: want Help, got {other:?}"),
        }
    }
    for argv in [
        vec!["lint", "--help"],
        vec!["--help", "lint"],
        vec!["clean", "-h"],
    ] {
        match parse(&args(&argv)) {
            Err(ArgsError::Help { .. }) => {}
            other => panic!("{argv:?}: want Help, got {other:?}"),
        }
    }
}

#[test]
fn strict_help_is_generated_from_the_same_grammar() {
    use clap::ValueEnum;
    let text = match parse(&args(&["--help"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("want Help, got {other:?}"),
    };
    for needle in [
        "dx",
        "lint",
        "build",
        "--workspace",
        "--output",
        "--fail-on",
        "Exit codes",
    ] {
        assert!(text.contains(needle), "top help missing {needle:?}");
    }
    for command in Command::value_variants() {
        assert!(
            text.contains(command.name()),
            "top help missing command {:?}",
            command.name()
        );
    }
    let grammar = super::super::grammar::cli_command();
    for long in [
        "workspace",
        "output",
        "fail-on",
        "dry-run",
        "quiet",
        "verbose",
        "color",
        "host",
        "log-level",
    ] {
        assert!(
            grammar
                .get_arguments()
                .any(|arg| arg.get_long() == Some(long)),
            "grammar missing --{long}"
        );
        assert!(text.contains(&format!("--{long}")), "help missing --{long}");
    }
    for needle in [
        "RUST_LOG",
        "NO_COLOR",
        "BUILD_WORKSPACE_DIRECTORY",
        "Environment:",
    ] {
        assert!(text.contains(needle), "top help missing env {needle:?}");
    }
    for argv in [vec!["lint", "--help"], vec!["status", "--help"]] {
        let text = match parse(&args(&argv)) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("{argv:?}: want Help, got {other:?}"),
        };
        for needle in [
            "Usage:",
            "Scopes:",
            "Exit codes:",
            "Output:",
            "Per-command flags:",
            "--workspace",
        ] {
            assert!(text.contains(needle), "{argv:?}: missing {needle:?}");
        }
    }
}

#[test]
fn strict_known_flags_on_wrong_commands_fail_as_unsupported() {
    for words in [
        vec!["build", "--serve"],
        vec!["lint", "--serve"],
        vec!["build", "--port=8080"],
        vec!["build", "--host=example.test"],
        vec!["build", "--open"],
        vec!["lint", "--min-coverage=80"],
        vec!["build", "--check"],
        vec!["lint", "--bazel"],
    ] {
        assert!(
            matches!(
                parse(&args(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?} must fail as unsupported, got {:?}",
            parse(&args(&words))
        );
    }
}

#[test]
fn strict_every_command_help_pins_usage_scopes_exits_output() {
    use clap::ValueEnum;
    for command in Command::value_variants() {
        let text = if *command == Command::Bazel {
            super::super::help::render_command_help(Command::Bazel)
        } else {
            let argv: Vec<String> = args(&[command.name(), "--help"]);
            match parse(&argv) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{:?}: want Help, got {other:?}", command.name()),
            }
        };
        for needle in [
            "Usage:",
            "Scopes:",
            "Exit codes:",
            "Output:",
            "Per-command flags:",
        ] {
            assert!(
                text.contains(needle),
                "{:?}: missing {needle:?}",
                command.name()
            );
        }
        assert!(
            text.contains(command.name()),
            "{:?}: missing command name",
            command.name()
        );
    }
    assert_eq!(Command::value_variants().len(), 33);
}

#[cfg(unix)]
#[test]
fn strict_non_utf8_argv_fails_as_invalid_scope() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let raw = OsString::from_vec(vec![0xff]);
    let lossy = raw.to_string_lossy().into_owned();
    assert!(raw.to_str().is_none(), "fixture must be non-UTF8");
    let argv = vec![OsString::from("lint"), raw.clone()];
    assert_eq!(
        parse(&argv),
        Err(ArgsError::InvalidScope {
            scope: lossy.clone(),
        })
    );
    let argv = vec![
        OsString::from("--workspace"),
        raw.clone(),
        OsString::from("lint"),
    ];
    assert_eq!(parse(&argv), Err(ArgsError::InvalidScope { scope: lossy }));
}
