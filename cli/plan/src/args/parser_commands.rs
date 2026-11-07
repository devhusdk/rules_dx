use super::super::command::owners::{AFFECTED_OWNERS, PROFILE_OWNERS};
use super::super::scope_error;
use super::super::{ArgsError, Command, OperationMode, assert_usage};
use super::parse;
use clap::ValueEnum;
use dx_output::{OutputMode, Threshold};
use dx_testing::cli::strings;

#[test]
fn command_option_ownership_rejects_every_unsupported_surface() {
    for command in [
        "security", "license", "update", "upgrade", "docs", "status", "init",
    ] {
        let base = match command {
            "upgrade" => vec![command, "--to=2.0.0"],
            _ => vec![command],
        };
        for option in ["--fail-on=error", "--report=junit=report.xml", "--bazel"] {
            let supported = match option {
                "--fail-on=error" => matches!(command, "security" | "license" | "update"),
                "--report=junit=report.xml" => command == "security" || command == "license",
                _ => false,
            };
            if supported {
                continue;
            }
            let mut words = base.clone();
            words.push(option);
            assert!(parse(&strings(&words)).is_err(), "{words:?}");
        }
        if !matches!(command, "generate" | "docs") {
            let mut words = base.clone();
            words.extend(["--", "--keep_going"]);
            assert!(parse(&strings(&words)).is_err(), "{words:?}");
        }
    }
    for words in [
        vec!["upgrade", "--to="],
        vec!["lint", "--pin=1.0.0"],
        vec!["build", "--pin=1.0.0"],
        vec!["clean", "--pin=1.0.0"],
        vec!["coverage", "--pin=1.0.0"],
        vec!["status", "extra"],
        vec!["init", "one", "two"],
        vec!["watch"],
        vec!["docs", ":relative"],
        vec!["docs", ""],
        vec!["generate", ":relative"],
        vec!["security", ""],
        vec!["update", ""],
        vec!["--bazel", "bazel", "status"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "{words:?}");
    }
}

const OWNED_OPTIONS: [(&str, &[&str], &[&str]); 9] = [
    ("--breaking-against", &["--breaking-against"], &["lint"]),
    ("--online", &["--online"], &["lint"]),
    (
        "--include-edits",
        &["--include-edits", "--output=json"],
        &["lint", "typecheck", "format", "check"],
    ),
    ("--min-coverage", &["--min-coverage=1"], &["coverage"]),
    (
        "--min-changed-line-coverage",
        &["--min-changed-line-coverage=1"],
        &["coverage"],
    ),
    ("--serve", &["--serve"], &["docs"]),
    ("--port", &["--serve", "--port=8080"], &["docs"]),
    ("--host", &["--serve", "--host=127.0.0.1"], &["docs"]),
    ("--open", &["--serve", "--open"], &["docs"]),
];

fn required_words(name: &str) -> &'static [&'static str] {
    match name {
        "upgrade" => &["--to=2.0.0"],
        "run" | "deploy" => &["//:demo"],
        "watch" => &["run", "//:demo"],
        "completion" => &["bash"],
        _ => &[],
    }
}

fn owned_option_words(command: Command, argv: &[&str]) -> Vec<String> {
    let name = command.name();
    [name]
        .into_iter()
        .chain(required_words(name).iter().copied())
        .chain(argv.iter().copied())
        .map(ToOwned::to_owned)
        .collect()
}

#[test]
fn an_owned_option_is_accepted_only_by_its_owner() {
    for command in Command::value_variants() {
        let command = *command;
        for (option, argv, owners) in OWNED_OPTIONS {
            let words = owned_option_words(command, argv);
            match parse(&words) {
                Ok(_) => assert!(
                    owners.contains(&command.name()),
                    "dx {} accepts {option}, which {owners:?} own: {words:?}",
                    command.name()
                ),
                Err(ArgsError::UnsupportedOption { option: named, .. })
                | Err(ArgsError::UnknownOption { option: named, .. }) => {
                    assert!(
                        !owners.contains(&command.name()),
                        "dx {} owns {option} but rejects it: {words:?}",
                        command.name()
                    );
                    if argv.len() == 1 {
                        assert_eq!(
                            named,
                            option,
                            "dx {} rejects {words:?} as {named}, which names another option",
                            command.name()
                        );
                    }
                }
                Err(error) => panic!("dx {words:?} failed with {error} instead of naming {option}"),
            }
        }
    }
}

#[test]
fn include_edits_needs_json_output() {
    for command in ["lint", "typecheck", "format", "check"] {
        for output in [None, Some("--output=text"), Some("--diff")] {
            let mut words = vec![command, "--include-edits"];
            words.extend(output);
            match parse(&strings(&words)) {
                Err(ArgsError::NeedsJson { option }) => assert_eq!(option, "--include-edits"),
                other => panic!("dx {words:?} gave {other:?} instead of NeedsJson"),
            }
        }
        let words = [command, "--include-edits", "--output=json"];
        let invocation = parse(&strings(&words)).expect("json accepts --include-edits");
        assert!(invocation.include_edits, "{words:?}");
        let invocation = parse(&strings(&[command, "--output=json"])).expect("json alone parses");
        assert!(!invocation.include_edits, "{command}");
    }
}

#[test]
fn unsafe_needs_apply_or_diff() {
    for command in ["lint", "typecheck", "format", "check"] {
        match parse(&strings(&[command, "--unsafe"])) {
            Err(ArgsError::UnsafeNeedsFixes) => {}
            other => panic!("dx {command} --unsafe gave {other:?} instead of UnsafeNeedsFixes"),
        }
        for fixes in ["--apply", "--diff"] {
            let invocation =
                parse(&strings(&[command, fixes, "--unsafe"])).expect("--unsafe with fixes");
            assert!(invocation.unsafe_fixes, "{command} {fixes}");
        }
        let invocation = parse(&strings(&[command, "--apply"])).expect("--apply alone parses");
        assert!(!invocation.unsafe_fixes, "{command}");
    }
    assert!(matches!(
        parse(&strings(&["watch", "lint", "--unsafe"])),
        Err(ArgsError::UnsafeNeedsFixes)
    ));
}

#[allow(clippy::type_complexity)]
const REGISTRY_GATES: [(&str, fn(Command) -> bool, &[&str]); 6] = [
    (
        "--affected",
        |c| AFFECTED_OWNERS.contains(&c),
        &["--affected"],
    ),
    ("--fail-on", Command::supports_fail_on, &["--fail-on=info"]),
    (
        "--fail-on",
        Command::supports_fail_on,
        &["--fail-on=warning"],
    ),
    ("--fail-on", Command::supports_fail_on, &["--fail-on=error"]),
    ("--debug", |c| PROFILE_OWNERS.contains(&c), &["--debug"]),
    ("--release", |c| PROFILE_OWNERS.contains(&c), &["--release"]),
];

#[test]
fn a_global_gate_is_accepted_exactly_where_the_registry_declares() {
    for command in Command::value_variants() {
        let command = *command;
        for (flag, supported, argv) in REGISTRY_GATES {
            let words = owned_option_words(command, argv);
            match parse(&words) {
                Ok(_) => assert!(
                    supported(command),
                    "dx {} accepts {flag} but the registry says it does not: {words:?}",
                    command.name()
                ),
                Err(ArgsError::UnsupportedOption { option: named, .. }) => {
                    assert_eq!(
                        named,
                        flag,
                        "dx {} rejects {words:?} as {named}, which names another option",
                        command.name()
                    );
                    assert!(
                        !supported(command),
                        "dx {} rejects {flag} but the registry says it accepts it: {words:?}",
                        command.name()
                    );
                }
                Err(error) => panic!("dx {words:?} failed with {error} instead of naming {flag}"),
            }
        }
    }
}

#[allow(clippy::type_complexity)]
const OUTPUT_MODES: [(&str, &str, fn(Command) -> bool); 2] = [
    ("--output=json", "json", Command::supports_json),
    ("--diff", "diff", Command::supports_diff),
];

#[test]
fn an_output_mode_is_gated_exactly_where_the_registry_declares_it() {
    for command in Command::value_variants() {
        let command = *command;
        for (flag, name, supported) in OUTPUT_MODES {
            let words = match command {
                Command::Watch => strings(&["watch", "lint", flag]),
                _ => owned_option_words(command, &[flag]),
            };
            match parse(&words) {
                Ok(got) => {
                    assert!(
                        supported(command),
                        "dx {} accepts {flag}, which the registry says it does not: {words:?}",
                        command.name()
                    );
                    let expected = if name == "diff" {
                        OutputMode::Diff
                    } else {
                        OutputMode::Json
                    };
                    assert_eq!(got.output, expected, "dx {} {words:?}", command.name());
                }
                Err(ArgsError::UnsupportedOption { option, .. }) => {
                    assert_eq!(
                        option,
                        flag,
                        "dx {} rejects {words:?} as {option}, which names another option",
                        command.name()
                    );
                    assert!(
                        !supported(command),
                        "dx {} rejects {flag} but the registry says it accepts it: {words:?}",
                        command.name()
                    );
                }
                Err(error) => panic!("dx {words:?} failed with {error} instead of naming {flag}"),
            }
        }
    }
}

#[test]
fn a_profile_flag_is_advertised_exactly_for_the_profile_owners() {
    use super::super::command::owners::PROFILE_OWNERS;
    for command in Command::value_variants() {
        let flags = super::super::grammar::advertised_flags(*command);
        for flag in ["debug", "release"] {
            assert_eq!(
                flags.iter().any(|name| name == flag),
                PROFILE_OWNERS.contains(command),
                "dx {} --{flag} help must match the profile owners",
                command.name()
            );
        }
    }
}

#[test]
fn every_command_rejects_empty_and_relative_scopes() {
    for command in Command::value_variants() {
        let command = *command;
        for scope in ["", ":target"] {
            let words = vec![command.name(), scope];
            assert_eq!(
                parse(&strings(&words)),
                Err(scope_error(scope)),
                "dx {} must reject the scope through the shared check: {words:?}",
                command.name()
            );
        }
    }
}

#[test]
fn update_with_a_selector_and_a_version_edits_that_package() {
    for (words, edits) in [
        (vec!["update", "cargo:anyhow", "1.2.3"], true),
        (
            vec!["update", "github-actions:actions/checkout", "v4"],
            true,
        ),
        (vec!["update", "maven:junit:junit", "4.13.3"], true),
        (
            vec!["update", "github-actions:actions/checkout", "deadbeefcafe"],
            true,
        ),
        (vec!["update"], false),
        (vec!["update", "cargo:anyhow"], false),
        (vec!["update", "cargo", "npm"], false),
        (vec!["update", "cargo:anyhow", "1.2.3", "npm:react"], false),
        (vec!["update", "cargo:anyhow", "//a:b"], false),
    ] {
        let got = parse(&strings(&words)).expect("parse update");
        assert_eq!(got.command, Command::Update);
        assert_eq!(got.edits_version(), edits, "{words:?}");
    }
    assert!(parse(&strings(&["upgrade", "cargo:anyhow", "1.2.3"])).is_err());
}

#[test]
fn upgrade_needs_a_to_version() {
    let upgrade_ok = parse(&strings(&["upgrade", "--to=2.0.0"])).expect("upgrade parses");
    assert_eq!(upgrade_ok.command, Command::Upgrade);
    assert_eq!(upgrade_ok.to, Some("2.0.0".to_owned()));
    assert_eq!(
        parse(&strings(&["build", "//a:one", "--to=2.0.0"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--to".to_owned(),
        })
    );
}

#[test]
fn upgrade_needs_to_with_no_scopes() {
    let got = parse(&strings(&["upgrade", "--to=2.0.0"])).expect("parse upgrade");
    assert_eq!(got.command, Command::Upgrade);
    assert!(got.command.is_adoption());
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&strings(&["upgrade", "--apply"])),
        Err(ArgsError::MissingValue {
            option: "--to <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["upgrade", "--to=2.0.0", "//a:one"])),
        Err(ArgsError::UnsupportedOption {
            command: "upgrade",
            option: "//a:one".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["upgrade", "--to=2.0.0", "--diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "upgrade",
            option: "--diff".to_owned(),
        })
    );
    let json = parse(&strings(&["upgrade", "--to=2.0.0", "--output=json"])).expect("upgrade json");
    assert_eq!(json.output, OutputMode::Json);
}

#[test]
fn run_rejects_machine_output_and_reports() {
    let got = parse(&strings(&["run", "//app:bin", "--output=json"])).expect("run json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&strings(&["run", "//app:bin", "--diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "run",
            option: "--diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["run", "--report=junit=out.xml"])),
        Err(ArgsError::UnsupportedOption {
            command: "run",
            option: "--report=junit=out.xml".to_owned(),
        })
    );
    let got = parse(&strings(&["run", "//app:bin", "--", "--port=8080"])).expect("parse run");
    assert_eq!(got.command, Command::Run);
    assert_eq!(got.targets, vec!["//app:bin".to_owned()]);
    assert_eq!(got.app_args(), ["--port=8080".to_owned()]);
    assert!(got.bazel_options.is_empty());
}

#[test]
fn run_bazel_options_route_apart_from_app_args() {
    let got = parse(&strings(&[
        "run",
        "--bazel-option=--jobs=4",
        "--bazel-option",
        "--config=ci",
        "//app:bin",
    ]))
    .expect("parse run");
    assert_eq!(got.bazel_options, strings(&["--jobs=4", "--config=ci"]));
    assert!(got.app_args().is_empty());
    let got = parse(&strings(&["run", "//a", "--", "--bazel-option=x"])).expect("app arg");
    assert!(got.bazel_options.is_empty());
    assert_eq!(got.app_args(), strings(&["--bazel-option=x"]));
    let got = parse(&strings(&[
        "run",
        "--bazel-option=--config=x",
        "//a",
        "--",
        "--y",
    ]))
    .expect("both");
    assert_eq!(got.bazel_options, strings(&["--config=x"]));
    assert_eq!(got.app_args(), strings(&["--y"]));
    for command in ["build", "deploy", "watch"] {
        assert!(
            matches!(
                parse(&strings(&[command, "--bazel-option=--jobs=4", "//a"])),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "{command}"
        );
    }
}

#[test]
fn output_contract_has_no_silent_ignore() {
    for command in [
        "lint",
        "typecheck",
        "format",
        "generate",
        "build",
        "test",
        "coverage",
        "run",
        "check",
        "security",
        "license",
        "update",
        "clean",
        "sync",
        "docs",
    ] {
        let got = parse(&strings(&[command, "--output=json"])).expect("json capable");
        assert_eq!(got.output, OutputMode::Json, "command: {command}");
        assert!(got.command.supports_json(), "command: {command}");
    }
    let got = parse(&strings(&[
        "update",
        "cargo:anyhow",
        "1.2.3",
        "--output=json",
    ]))
    .expect("update version json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&strings(&["upgrade", "--to=2.0.0", "--output=json"])).expect("upgrade json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&strings(&["status", "--output=json"])).expect("status json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Status.supports_json());
    for (words, want) in [
        (vec!["watch", "test", "--output=json"], OutputMode::Json),
        (vec!["watch", "build", "-o", "json"], OutputMode::Json),
        (vec!["watch", "lint", "--diff"], OutputMode::Diff),
        (vec!["watch", "check", "--diff"], OutputMode::Diff),
        (vec!["watch", "--diff", "format"], OutputMode::Diff),
    ] {
        let got = parse(&strings(&words)).expect("the wrapped command takes the mode");
        assert_eq!(got.command, Command::Watch, "{words:?}");
        assert_eq!(got.output, want, "{words:?}");
    }
    for command in ["lint", "typecheck", "format", "generate", "check"] {
        let got = parse(&strings(&[command, "--diff"])).expect("diff capable");
        assert_eq!(got.output, OutputMode::Diff, "command: {command}");
        assert!(got.command.supports_diff(), "command: {command}");
    }
    for words in [
        vec!["clean", "--diff"],
        vec!["sync", "--diff"],
        vec!["run", "//app:bin", "--diff"],
        vec!["init", "proj", "--diff"],
        vec!["watch", "build", "--diff"],
        vec!["watch", "run", "//app:bin", "--diff"],
        vec!["completion", "bash", "--output=json"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?}"
        );
    }
    for words in [
        vec!["build", "//a:one", "--diff"],
        vec!["test", "//a:one", "--diff"],
        vec!["coverage", "--diff"],
        vec!["run", "//a:one", "--diff"],
        vec!["security", "--diff"],
        vec!["license", "--diff"],
        vec!["update", "--diff"],
        vec!["status", "--diff"],
        vec!["clean", "--diff"],
        vec!["sync", "--diff"],
        vec!["docs", "--diff"],
        vec!["update", "cargo:anyhow", "1.2.3", "--diff"],
        vec!["upgrade", "--to=2.0.0", "--diff"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--diff".to_owned(),
            }),
            "words: {words:?}"
        );
    }
}

#[test]
fn bazel_is_an_unknown_command() {
    for words in [
        vec!["bazel", "build", "//...", "--", "--jobs=4"],
        vec!["bazel", "--jobs=4"],
        vec!["--output=json", "bazel", "status"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "words: {words:?}");
    }
}

#[test]
fn test_and_coverage_keep_test_arg_passthrough_verbatim() {
    for command in ["test", "coverage"] {
        let got = parse(&strings(&[
            command,
            "//a:one",
            "--",
            "--test_arg=--exact",
            "--test_arg",
            "case with spaces héllo",
            "--test_filter=unit",
        ]))
        .expect("parse");
        assert_eq!(
            got.bazel_options,
            strings(&[
                "--test_arg=--exact",
                "--test_arg",
                "case with spaces héllo",
                "--test_filter=unit",
            ]),
            "{command}"
        );
    }
}

#[test]
fn version_flags_stay_with_their_commands() {
    for words in [
        vec!["status", "--serve"],
        vec!["test", "//a:one", "--to=0.1.0"],
        vec!["lint", "--serve"],
        vec!["status", "--to=0.1.0"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?}"
        );
    }
}

#[test]
fn sync_rejects_the_removed_shell_flag() {
    assert!(parse(&strings(&["sync", "--shell=direnv"])).is_err());
}

#[test]
fn check_defaults_to_check_and_applies_only_with_apply() {
    for command in ["lint", "typecheck", "format", "check"] {
        let bare = parse(&strings(&[command])).expect("bare parses");
        assert!(!bare.applies(), "{command} checks without --apply");
        assert_eq!(bare.operation(), OperationMode::Check, "{command}");
        let applied = parse(&strings(&[command, "--apply"])).expect("apply parses");
        assert!(applied.applies(), "{command}");
    }
}

#[test]
fn docs_check_serve_port_parse() {
    let got = parse(&strings(&["docs"])).expect("bare docs parses");
    assert_eq!(got.command, Command::Docs);
    assert!(!got.serve);
    assert_eq!(got.port, None);
    assert_eq!(got.host, None);
    assert!(!got.open);
    let got = parse(&strings(&["docs", "--serve"])).expect("serve parses");
    assert!(got.serve);
    assert_eq!(got.port, None);
    let got = parse(&strings(&["docs", "--serve", "--port=8080"])).expect("port parses");
    assert!(got.serve);
    assert_eq!(got.port, Some(8080));
    let got = parse(&strings(&["docs", "--serve", "--port", "9000"])).expect("split port");
    assert!(got.serve);
    assert_eq!(got.port, Some(9000));
    let got = parse(&strings(&["docs", "--serve", "--host=example.test"])).expect("host parses");
    assert_eq!(got.host, Some("example.test".to_owned()));
    let got = parse(&strings(&["docs", "--serve", "--host", "example.test"])).expect("split host");
    assert_eq!(got.host, Some("example.test".to_owned()));
    let got = parse(&strings(&["docs", "--serve", "--open"])).expect("open parses");
    assert!(got.open);
    for words in [
        vec!["docs", "--port=8080"],
        vec!["docs", "--port"],
        vec!["docs", "--port=notanumber"],
        vec!["docs", "--port="],
        vec!["docs", "--port=0"],
        vec!["docs", "--host=example.test"],
        vec!["docs", "--host"],
        vec!["docs", "--host="],
        vec!["docs", "--open"],
        vec!["build", "--serve"],
        vec!["lint", "--serve"],
        vec!["build", "--port=8080"],
        vec!["build", "--host=example.test"],
        vec!["build", "--open"],
        vec!["docs", "--fail-on=error"],
        vec!["docs", "--report=sarif=out.sarif"],
        vec!["docs", "--to=0.1.0"],
        vec!["docs", "--diff"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
                    | Err(ArgsError::MissingValue { .. })
                    | Err(ArgsError::UnknownOption { .. })
                    | Err(ArgsError::Usage { .. })
            ),
            "words: {words:?}"
        );
    }
    let got = parse(&strings(&["docs", "--", "--jobs=4"])).expect("docs forwards command options");
    assert_eq!(got.bazel_options, strings(&["--jobs=4"]));
    let got = parse(&strings(&["docs", "--", "--config=ci"])).expect("docs forwards");
    assert_eq!(got.bazel_options, strings(&["--config=ci"]));
}

#[test]
fn color_parses_globally_with_bad_values_rejected() {
    use dx_output::ColorMode;
    let got = parse(&strings(&["lint"])).expect("default color");
    assert_eq!(got.color, ColorMode::Auto);
    let got = parse(&strings(&["lint", "--color=never"])).expect("never parses");
    assert_eq!(got.color, ColorMode::Never);
    let got = parse(&strings(&["lint", "--color=always"])).expect("always parses");
    assert_eq!(got.color, ColorMode::Always);
    assert_eq!(
        parse(&strings(&["lint", "--color=bright"])),
        Err(ArgsError::BadColor {
            value: "bright".to_owned(),
        })
    );
    assert_usage(
        ["lint", "--color"],
        parse(&strings(&["lint", "--color"])).unwrap_err(),
        &["--color"],
    );
}

#[test]
fn managed_commands_parse_repo_and_exact_scopes() {
    {
        let command = "sync";
        let got = parse(&strings(&[command])).expect("parse");
        assert!(got.command.is_managed());
        assert!(got.targets.is_empty());
        assert!(got.bazel_options.is_empty());
        let got = parse(&strings(&[command, "//a:one", "--", "--jobs=4"])).expect("scoped parse");
        assert_eq!(got.targets, strings(&["//a:one"]));
        assert_eq!(got.bazel_options, strings(&["--jobs=4"]));
        let got = parse(&strings(&[command, "@repo//pkg:lib"])).expect("external label");
        assert_eq!(got.targets, strings(&["@repo//pkg:lib"]));
    }
    assert_eq!(
        parse(&strings(&["sync", "//a:one", "//b:two"])),
        Err(ArgsError::UnsupportedOption {
            command: "sync",
            option: "//b:two".to_owned(),
        })
    );
    for words in [
        vec!["sync", "//a/..."],
        vec!["sync", ":target"],
        vec!["sync", ""],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::EmptyScope)
                    | Err(ArgsError::RelativeLabel { .. })
                    | Err(ArgsError::InvalidScope { .. })
                    | Err(ArgsError::UnknownOption { .. })
                    | Err(ArgsError::Usage { .. })
            ),
            "words: {words:?}"
        );
    }
    for words in [
        vec!["sync", "--fail-on=error"],
        vec!["sync", "--report=sarif=out.sarif"],
        vec!["sync", "--to=0.1.0"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?}"
        );
    }
    {
        let command = "sync";
        let got = parse(&strings(&[command])).expect("bare parses");
        assert_eq!(got.operation(), OperationMode::Apply);
        assert!(got.applies());
        assert!(parse(&strings(&[command, "--apply"])).is_err());
    }
}

#[test]
fn owners_deps_and_why_are_unknown_commands() {
    for words in [
        vec!["owners", "//a:one"],
        vec!["deps", "--configured", "//a:one"],
        vec!["why", "src/a.rs", "//a:one"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "words: {words:?}");
    }
}

#[test]
fn init_has_no_overwrite_flag() {
    for words in [
        vec!["init", "--overwrite"],
        vec!["init", "--overwrite", "demo"],
    ] {
        assert_usage(
            &words,
            parse(&strings(&words)).unwrap_err(),
            &["--overwrite"],
        );
    }
}

#[test]
fn watch_takes_here_and_affected_as_the_wrapped_command_does() {
    let got = parse(&strings(&["watch", "test", "--here"])).expect("here parses");
    assert_eq!(got.command, Command::Watch);
    assert!(got.here);
    assert_eq!(got.targets, vec!["test".to_owned()]);
    let got = parse(&strings(&["watch", "lint", "--affected"])).expect("affected parses");
    assert_eq!(got.command, Command::Watch);
    assert!(got.affected);
    assert_eq!(got.targets, vec!["lint".to_owned()]);
    assert!(matches!(
        parse(&strings(&["watch", "run", "//:demo", "--here"])),
        Err(ArgsError::UnsupportedOption { option, .. }) if option == "--here"
    ));
}

#[test]
fn watch_defers_fail_on_to_the_wrapped_command() {
    let got = parse(&strings(&["watch", "lint", "--fail-on=error"])).expect("fail-on parses");
    assert_eq!(got.command, Command::Watch);
    assert_eq!(got.fail_on, Threshold::Error);
    assert_eq!(got.targets[0], "lint");
    assert!(matches!(
        parse(&strings(&["watch", "build", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption { command, option })
            if command == "build" && option == "--fail-on"
    ));
    assert!(matches!(
        parse(&strings(&["watch", "--fail-on=error"])),
        Err(ArgsError::MissingValue { .. })
    ));
    let got = parse(&strings(&["watch", "docs", "--fail-on=error"]))
        .expect("the runtime rejects a non-watchable target");
    assert_eq!(got.targets[0], "docs");
}

#[test]
fn here_selects_cwd_scope_only_via_explicit_flag() {
    for command in [
        "security",
        "license",
        "lint",
        "typecheck",
        "format",
        "generate",
        "build",
        "test",
        "coverage",
        "check",
        "docs",
    ] {
        let got = parse(&strings(&[command, "--here"])).expect("here parses");
        assert!(got.here, "command: {command}");
        assert!(got.targets.is_empty(), "command: {command}");
        assert!(got.command.supports_here(), "command: {command}");
        assert!(
            parse(&strings(&[command, "--cwd"])).is_err(),
            "command: {command}"
        );
        let bare = parse(&strings(&[command])).expect("bare parses");
        assert!(!bare.here, "command: {command}");
        assert!(bare.targets.is_empty(), "command: {command}");
    }
    for words in [
        vec!["build", "--here", "//a:one"],
        vec!["lint", "src/a.py", "--here"],
        vec!["test", "--here", "//..."],
        vec!["security", "--here", "//a:one"],
        vec!["license", "cli/cli", "--here"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::ConflictingHere),
            "words: {words:?}"
        );
    }
    for words in [
        vec!["clean", "--here"],
        vec!["update", "--here"],
        vec!["update", "cargo:anyhow", "1.2.3", "--here"],
        vec!["sync", "--here"],
        vec!["run", "//app:bin", "--here"],
        vec!["deploy", "//app:bin", "--here"],
        vec!["status", "--here"],
        vec!["completion", "bash", "--here"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--here".to_owned(),
            }),
            "words: {words:?}"
        );
    }
}

#[test]
fn completion_takes_exactly_one_shell() {
    let plain = parse(&strings(&["completion", "bash"])).expect("plain renders");
    assert_eq!(plain.command, Command::Completion);
    assert_eq!(plain.targets, vec!["bash".to_owned()]);
    assert_eq!(
        parse(&strings(&["completion", "bash", "zsh"])),
        Err(ArgsError::UnsupportedOption {
            command: "completion",
            option: "zsh".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["completion"])),
        Err(ArgsError::MissingValue {
            option: "<shell>".to_owned(),
        })
    );
}

#[test]
fn offline_and_frozen_are_independent_policies_on_audit_and_update() {
    for command in ["security", "license", "update"] {
        let got = parse(&strings(&[command, "--offline"])).expect("offline parses");
        assert!(got.offline, "command: {command}");
        assert!(!got.frozen, "command: {command}");
        let frozen = parse(&strings(&[command, "--frozen"])).expect("frozen parses");
        assert!(frozen.frozen, "command: {command}");
        assert!(!frozen.offline, "command: {command}");
        let both = parse(&strings(&[command, "--offline", "--frozen"])).expect("combined parses");
        assert!(both.offline && both.frozen, "command: {command}");
        let bare = parse(&strings(&[command])).expect("bare parses");
        assert!(!bare.offline, "command: {command}");
        assert!(!bare.frozen, "command: {command}");
    }
    let update = parse(&strings(&["update", "cargo:anyhow", "1.2.3", "--offline"]))
        .expect("version offline");
    assert!(update.offline);
    assert!(!update.frozen);
    let update_frozen =
        parse(&strings(&["update", "cargo:anyhow", "1.2.3", "--frozen"])).expect("version frozen");
    assert!(update_frozen.frozen);
    assert!(!update_frozen.offline);
    for words in [
        vec!["lint", "--offline"],
        vec!["build", "//a:one", "--offline"],
        vec!["clean", "--offline"],
        vec!["sync", "--offline"],
        vec!["status", "--offline"],
        vec!["docs", "--offline"],
        vec!["upgrade", "--to=2.0.0", "--offline"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--offline".to_owned(),
            }),
            "words: {words:?}"
        );
    }
    for words in [
        vec!["lint", "--frozen"],
        vec!["build", "//a:one", "--frozen"],
        vec!["clean", "--frozen"],
        vec!["status", "--frozen"],
        vec!["docs", "--frozen"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--frozen".to_owned(),
            }),
            "words: {words:?}"
        );
    }
    assert_usage(
        ["security", "--offline=yes"],
        parse(&strings(&["security", "--offline=yes"])).unwrap_err(),
        &["--offline"],
    );
}

#[test]
fn a_refused_output_diff_is_named_before_any_other_flag_the_command_refuses() {
    for (command, also_refused) in [
        ("clean", "--report=junit=one.xml"),
        ("update", "--report=junit=one.xml"),
        ("upgrade", "--report=junit=one.xml"),
        ("sync", "--report=junit=one.xml"),
        ("docs", "--port=1"),
    ] {
        let words = owned_option_words(
            Command::parse(command).unwrap_or_else(|| panic!("registry names {command}")),
            &["--diff", also_refused],
        );
        assert_eq!(
            parse(&words),
            Err(ArgsError::UnsupportedOption {
                command,
                option: "--diff".to_owned(),
            }),
            "words: {words:?}"
        );
    }
    assert_eq!(
        parse(&[
            "clean".to_owned(),
            "--report=junit=one.xml".to_owned(),
            "--report=sarif=two.sarif".to_owned(),
        ]),
        Err(ArgsError::UnsupportedOption {
            command: "clean",
            option: "--report=junit=one.xml".to_owned(),
        })
    );
}

#[test]
fn clean_names_its_scope_before_the_passthrough_it_also_refuses() {
    for (words, option) in [
        (
            vec!["clean", "//:demo", "--", "--config=ci"],
            "//:demo".to_owned(),
        ),
        (
            vec!["upgrade", "--to=2.0.0", "--", "--config=ci"],
            "--".to_owned(),
        ),
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option,
            }),
            "words: {words:?}"
        );
    }
}

#[test]
fn worktree_subcommands_parse_with_their_flags() {
    let got = parse(&strings(&[
        "worktree",
        "add",
        "topic",
        "--create=origin/main",
    ]))
    .expect("add");
    assert_eq!(got.command, Command::Worktree);
    assert_eq!(got.targets, vec!["add", "topic"]);
    assert_eq!(got.create.as_deref(), Some("origin/main"));
    let got = parse(&strings(&["worktree", "add", "topic", "--path-only"])).expect("path");
    assert!(got.path_only);
    let got = parse(&strings(&["worktree", "release", "--force"])).expect("release");
    assert!(got.force);
    let got = parse(&strings(&["worktree", "prune", "--apply"])).expect("prune");
    assert!(got.apply);
    assert!(!got.disk);
    assert!(!got.bases);
    let got = parse(&strings(&["worktree", "prune", "--apply", "--bases"])).expect("bases");
    assert!(got.bases && got.apply);
    let got = parse(&strings(&["status", "--capabilities"])).expect("status");
    assert!(got.capabilities);
    assert!(!parse(&strings(&["status"])).expect("status").capabilities);
    assert_eq!(
        parse(&strings(&["lint", "--capabilities"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--capabilities".to_owned(),
        })
    );
    let got = parse(&strings(&["status", "--files"])).expect("status --files");
    assert!(got.files);
    assert!(!parse(&strings(&["status"])).expect("status").files);
    assert!(parse(&strings(&["lint", "--files"])).is_err());
    let got = parse(&strings(&["worktree", "list", "--disk"])).expect("list --disk");
    assert!(got.disk);
    let got = parse(&strings(&["worktree", "--disk"])).expect("bare --disk");
    assert!(got.disk);
    let got = parse(&strings(&["worktree", "list"])).expect("list");
    assert!(!got.disk);
    assert!(parse(&strings(&["worktree"])).is_ok());
    assert!(parse(&strings(&["worktree", "hook", "guard"])).is_ok());
    assert!(parse(&strings(&["worktree", "hook", "stop"])).is_ok());
    let got = parse(&strings(&["worktree", "add", "topic", "--force"])).expect("add force");
    assert!(got.force);
}

#[test]
fn worktree_hook_rejects_unknown_hook_names() {
    for name in ["bogus", "remvoe", "delete"] {
        let error = parse(&strings(&["worktree", "hook", name])).expect_err(name);
        assert_eq!(
            error.to_string(),
            format!("unknown dx worktree hook {name:?}: want one of: create, remove, stop, guard")
        );
    }
}

#[test]
fn worktree_rejects_bad_subcommands_and_arity() {
    for words in [
        vec!["worktree", "bogus"],
        vec!["worktree", "add"],
        vec!["worktree", "add", "a", "b"],
        vec!["worktree", "list", "extra"],
        vec!["worktree", "hook", "other"],
        vec!["worktree", "list", "--", "--jobs=1"],
        vec!["worktree", "list", "--disk=yes"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "words: {words:?}");
    }
}

#[test]
fn generate_refuses_external_labels_but_takes_main_repo_forms() {
    assert_eq!(
        parse(&strings(&["generate", "@ext//p"])),
        Err(ArgsError::InvalidScope {
            scope: "@ext//p".to_owned()
        })
    );
    for scope in ["@//p:x", "@@//p/..."] {
        parse(&strings(&["generate", scope])).unwrap_or_else(|e| panic!("{scope}: {e:?}"));
    }
}

#[test]
fn init_rejects_with_and_full() {
    for name in ["with", "full"] {
        let flag = format!("--{name}");
        assert!(parse(&strings(&["init", &flag])).is_err(), "{flag}");
    }
}

#[test]
fn profile_parses_on_build_test_coverage_and_run() {
    for command in ["build", "test", "coverage", "run"] {
        assert!(
            parse(&strings(&[command, "--timings"]))
                .expect(command)
                .build_profile
        );
        assert!(!parse(&strings(&[command])).expect(command).build_profile);
    }
    assert!(parse(&strings(&["lint", "--timings"])).is_err());
}

#[test]
fn explain_is_not_a_command_or_a_flag() {
    for words in [
        vec!["explain"],
        vec!["build", "--explain"],
        vec!["test", "--explain"],
        vec!["coverage", "--explain"],
        vec!["check", "--explain"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "{words:?}");
    }
}

#[test]
fn floor_and_reason_parse_only_on_an_update_with_a_version() {
    let got = parse(&strings(&[
        "update",
        "bazel:rules_cc",
        "0.2.0",
        "--floor",
        "--reason",
        "oldest passing",
    ]))
    .expect("update floor");
    assert!(got.floor);
    assert!(got.edits_version());
    assert_eq!(got.reason.as_deref(), Some("oldest passing"));
    let bare = parse(&strings(&["update", "bazel:rules_cc", "0.2.0"])).expect("update version");
    assert!(!bare.floor);
    assert_eq!(bare.reason, None);
    for words in [
        vec!["update", "--floor"],
        vec!["update", "cargo:anyhow", "--floor"],
        vec!["build", "bazel:rules_cc", "0.2.0", "--floor"],
        vec!["update", "--reason", "x"],
        vec!["build", "//a:one", "--reason", "x"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. } | ArgsError::UnknownOption { .. })
            ),
            "{words:?}"
        );
    }
}

#[test]
fn update_pr_takes_group_and_labels() {
    let got = parse(&strings(&[
        "update",
        "--pr",
        "--group=all",
        "--labels=deps,bot",
        "cargo",
    ]))
    .expect("pr");
    assert!(got.pr);
    assert_eq!(got.group.as_deref(), Some("all"));
    assert_eq!(got.labels.as_deref(), Some("deps,bot"));
    let got = parse(&strings(&[
        "update",
        "--pr",
        "--automerge=minor",
        "--min-age=0",
        "--ignore=cargo:serde",
        "--ignore=npm:left-pad",
        "--allow-major",
        "--prereleases",
        "--max-prs=2",
    ]))
    .expect("policy");
    assert_eq!(got.automerge.as_deref(), Some("minor"));
    assert_eq!(got.min_age, Some(0));
    assert_eq!(got.ignore, ["cargo:serde", "npm:left-pad"]);
    assert!(got.allow_major);
    assert!(got.prereleases);
    assert_eq!(got.max_prs, Some(2));
    assert!(!parse(&strings(&["update"])).expect("bare").pr);
    for words in [
        vec!["update", "--pr", "--outdated"],
        vec!["update", "--pr", "--apply"],
        vec!["update", "--pr", "--floor"],
        vec!["update", "--pr", "--reason", "x"],
        vec!["update", "--pr", "--offline"],
        vec!["update", "--pr", "cargo:anyhow", "1.0.0"],
        vec!["update", "--group=set"],
        vec!["update", "--labels=deps"],
        vec!["update", "--automerge=all"],
        vec!["update", "--min-age=1"],
        vec!["update", "--ignore=cargo:serde"],
        vec!["update", "--allow-major"],
        vec!["update", "--prereleases"],
        vec!["update", "--max-prs=2"],
        vec!["update", "--pr", "--automerge=some"],
        vec!["update", "--pr", "--ignore=serde"],
        vec!["update", "--pr", "--max-prs=0"],
        vec!["update", "--pr", "--group=team"],
        vec!["build", "--pr"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "{words:?}");
    }
}

#[test]
fn update_outdated_is_a_read_only_listing() {
    let got = parse(&strings(&["update", "--outdated", "cargo"])).expect("outdated");
    assert!(got.outdated);
    assert!(!got.edits_version());
    assert!(!parse(&strings(&["update"])).expect("bare").outdated);
    for words in [
        vec!["update", "--outdated", "--apply"],
        vec!["update", "--outdated", "--floor"],
        vec!["update", "--outdated", "--reason", "x"],
        vec!["update", "--outdated", "cargo:anyhow", "1.0.0"],
        vec!["build", "--outdated"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. } | ArgsError::UnknownOption { .. })
            ),
            "{words:?}"
        );
    }
}

#[test]
fn unsupported_flag_names_the_commands_that_take_it() {
    assert!(parse(&strings(&["lint", "--affected"])).is_ok());
    let err = parse(&strings(&["run", "--affected"]))
        .expect_err("run takes no --affected")
        .to_string();
    assert!(err.contains("use it with dx"), "{err}");
    assert!(err.contains("test"), "{err}");
    assert_eq!(
        super::super::unsupported_hint("run", "--nope"),
        "run dx run --help"
    );
}

#[test]
fn update_prune_needs_apply() {
    let got = parse(&strings(&["update", "--apply", "--prune", "cargo"])).expect("prune");
    assert!(got.prune && got.apply);
    assert!(
        !parse(&strings(&["update", "--apply"]))
            .expect("apply")
            .prune
    );
    for words in [
        vec!["update", "--prune"],
        vec!["update", "--apply", "--prune", "--frozen"],
        vec!["update", "--pr", "--apply", "--prune"],
        vec!["build", "--prune"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "{words:?}");
    }
}

#[test]
fn update_add_missing_needs_apply_and_network() {
    let got = parse(&strings(&["update", "--apply", "--add-missing", "cargo"])).expect("add");
    assert!(got.add_missing && got.apply);
    assert!(
        !parse(&strings(&["update", "--apply"]))
            .expect("apply")
            .add_missing
    );
    for words in [
        vec!["update", "--add-missing"],
        vec!["update", "--apply", "--add-missing", "--frozen"],
        vec!["update", "--apply", "--add-missing", "--offline"],
        vec!["update", "--pr", "--apply", "--add-missing"],
        vec!["build", "--add-missing"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "{words:?}");
    }
}

#[test]
fn release_push_needs_apply() {
    assert!(parse(&strings(&["release", "--push"])).is_err());
    assert!(parse(&strings(&["release", "--apply", "--push"])).is_ok());
}
