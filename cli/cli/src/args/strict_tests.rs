use super::super::{assert_usage, ArgsError, Command};
use super::parse;
use crate::test_support::strings;

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
        let token = words.last().expect("a token to reject");
        let flag = token.split('=').next().expect("a flag");
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[flag]);
    }
}

#[test]
fn strict_missing_values_fail_with_bare_flag() {
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
    assert_usage(
        &["coverage", "--min-coverage"],
        parse(&strings(&["coverage", "--min-coverage"])).unwrap_err(),
        &["--min-coverage"],
    );
    assert_usage(
        &["docs", "--port"],
        parse(&strings(&["docs", "--port"])).unwrap_err(),
        &["--port"],
    );
    assert_usage(
        &["docs", "--host"],
        parse(&strings(&["docs", "--host"])).unwrap_err(),
        &["--host"],
    );
    assert_usage(
        &["lint", "--color"],
        parse(&strings(&["lint", "--color"])).unwrap_err(),
        &["--color"],
    );
    assert_usage(
        &["migrate", "--from", "--to=2.0.0"],
        parse(&strings(&["migrate", "--from", "--to=2.0.0"])).unwrap_err(),
        &["--from"],
    );
}

#[test]
fn strict_hyphen_values_are_never_consumed_as_option_values() {
    assert_usage(
        &["lint", "--output", "--quiet"],
        parse(&strings(&["lint", "--output", "--quiet"])).unwrap_err(),
        &["--output"],
    );
    assert_usage(
        &["lint", "--workspace", "--quiet"],
        parse(&strings(&["lint", "--workspace", "--quiet"])).unwrap_err(),
        &["--workspace"],
    );
    assert_usage(
        &["build", "--output", "--dry-run"],
        parse(&strings(&["build", "--output", "--dry-run"])).unwrap_err(),
        &["--output"],
    );
}

#[test]
fn strict_bad_values_fail_with_contract_shapes() {
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
    for words in [
        vec!["coverage", "--min-coverage=eighty"],
        vec!["coverage", "--min-coverage=101"],
    ] {
        assert_usage(
            &words,
            parse(&strings(&words)).unwrap_err(),
            &["min-coverage"],
        );
    }
    assert_eq!(
        parse(&strings(&["lint", "--color=bright"])),
        Err(ArgsError::BadColor {
            value: "bright".to_owned(),
        })
    );
    assert_usage(
        &["docs", "--serve", "--port=0"],
        parse(&strings(&["docs", "--serve", "--port=0"])).unwrap_err(),
        &["--port"],
    );
}

#[test]
fn strict_typo_suggestions_come_from_the_same_grammar() {
    for (words, needle) in [
        (vec!["lintt"], "subcommands exist"),
        (vec!["lint", "--ouptut=json"], "--output"),
    ] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[needle]);
    }
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
        let got =
            parse(&strings(&words)).unwrap_or_else(|error| panic!("words: {words:?}: {error}"));
        assert_eq!(got.command, command, "words: {words:?}");
    }
    assert!(
        parse(&strings(&["build", "--here", "--cwd"]))
            .expect("here")
            .here
    );
    assert!(parse(&strings(&["build", "-vv"])).expect("verbose").verbose);
    assert!(
        parse(&strings(&["build", "--debug", "--release"])).is_err(),
        "repeatable flags must not weaken the profile conflict"
    );
}

#[test]
fn strict_bazel_tail_forwards_verbatim() {
    let got = parse(&strings(&["bazel", "build", "//...", "--", "--jobs=4"])).expect("parse");
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
        let got = parse(&strings(&words)).expect("verbatim");
        assert_eq!(got.command, Command::Bazel, "words: {words:?}");
    }
    for words in [
        vec!["--output=json", "bazel", "version"],
        vec!["--check", "bazel", "version"],
        vec!["--here", "bazel", "version"],
    ] {
        assert_usage(
            &words,
            parse(&strings(&words)).unwrap_err(),
            &[words[0].split('=').next().expect("a flag")],
        );
    }
}

#[test]
fn strict_typed_values_map_to_dx_errors_in_either_flag_order() {
    for words in [
        vec!["build", "--debug", "--release"],
        vec!["build", "--release", "--debug"],
    ] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &["--debug"]);
    }
    for words in [
        vec!["lint", "--verbose", "--log-level=debug"],
        vec!["lint", "--log-level=debug", "--verbose"],
        vec!["lint", "-v", "--log-level=info"],
        vec!["lint", "--log-level=info", "-v"],
    ] {
        assert_usage(
            &words,
            parse(&strings(&words)).unwrap_err(),
            &["--log-level"],
        );
    }
    for (words, needle) in [
        (vec!["coverage", "--min-coverage=eighty"], "min-coverage"),
        (vec!["coverage", "--min-coverage=101"], "min-coverage"),
        (vec!["coverage", "--min-coverage=-1"], "min-coverage"),
        (vec!["lint", "--report=sarif"], "report"),
        (vec!["lint", "--report=sarif="], "report"),
        (vec!["docs", "--serve", "--port=0"], "port"),
        (vec!["docs", "--serve", "--port=notanumber"], "port"),
        (vec!["docs", "--serve", "--port="], "port"),
        (vec!["docs", "--serve", "--host="], "host"),
        (vec!["version", "--pin="], "pin"),
        (vec!["migrate", "--from="], "from"),
        (vec!["migrate", "--to="], "to"),
    ] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[needle]);
    }
}

#[test]
fn strict_here_conflicts_stay_imperative_and_ordered() {
    for words in [
        vec!["lint", "--here", "//pkg:target"],
        vec!["build", "--here", "//pkg:target"],
        vec!["security", "--here", "//pkg:target"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::ConflictingHere),
            "words: {words:?}"
        );
    }
    assert!(
        matches!(
            parse(&strings(&["version", "--here", "//pkg:target"])),
            Err(ArgsError::UnsupportedOption { .. })
        ),
        "the command gate must outrank the scope conflict"
    );
}

#[test]
fn strict_help_verb_redirects_to_generated_help() {
    for words in [vec!["Help"], vec!["HELP"]] {
        assert_usage(&words, parse(&strings(&words)).unwrap_err(), &[words[0]]);
    }
    for words in [vec!["help"], vec!["help", "lint"], vec!["help", "status"]] {
        match parse(&strings(&words)) {
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
    let verb = match parse(&strings(&["help", "lint"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("help lint: want Help, got {other:?}"),
    };
    let flag = match parse(&strings(&["lint", "--help"])) {
        Err(ArgsError::Help { text }) => text,
        other => panic!("lint --help: want Help, got {other:?}"),
    };
    assert_eq!(verb, flag, "help verb must redirect to per-command help");
    for flag in ["--help", "-h"] {
        match parse(&strings(&[flag])) {
            Err(ArgsError::Help { .. }) => {}
            other => panic!("{flag}: want Help, got {other:?}"),
        }
    }
    for argv in [
        vec!["lint", "--help"],
        vec!["--help", "lint"],
        vec!["clean", "-h"],
    ] {
        match parse(&strings(&argv)) {
            Err(ArgsError::Help { .. }) => {}
            other => panic!("{argv:?}: want Help, got {other:?}"),
        }
    }
}

#[test]
fn strict_help_is_generated_from_the_same_grammar() {
    use clap::ValueEnum;
    let text = match parse(&strings(&["--help"])) {
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
    const GLOBAL_LONGS: [&str; 7] = [
        "workspace",
        "output",
        "dry-run",
        "quiet",
        "verbose",
        "color",
        "log-level",
    ];
    for command in Command::value_variants() {
        let sub = grammar
            .find_subcommand(command.name())
            .unwrap_or_else(|| panic!("dx {} must be a subcommand", command.name()));
        for long in GLOBAL_LONGS {
            assert!(
                sub.get_arguments().any(|arg| arg.get_long() == Some(long)),
                "dx {} grammar missing --{long}",
                command.name()
            );
        }
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
        let text = match parse(&strings(&argv)) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("{argv:?}: want Help, got {other:?}"),
        };
        for needle in [
            "Usage:",
            "Scopes:",
            "Exit codes:",
            "Output:",
            "Per-command flags:",
        ] {
            assert!(text.contains(needle), "{argv:?}: missing {needle:?}");
        }
        for long in GLOBAL_LONGS {
            assert!(
                text.contains(&format!("--{long}")),
                "{argv:?}: missing --{long}"
            );
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
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?} must fail as unsupported, got {:?}",
            parse(&strings(&words))
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
            let argv: Vec<String> = strings(&[command.name(), "--help"]);
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
        OsString::from("lint"),
        OsString::from("--workspace"),
        raw.clone(),
    ];
    assert_eq!(parse(&argv), Err(ArgsError::InvalidScope { scope: lossy }));
}
