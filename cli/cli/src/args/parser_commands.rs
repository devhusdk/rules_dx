use super::super::scope_error;
use super::super::{assert_usage, ArgsError, Command, OperationMode};
use super::parse;
use crate::test_support::strings;
use clap::ValueEnum;
use dx_output::OutputMode;

#[test]
fn command_option_ownership_rejects_every_unsupported_surface() {
    for command in [
        "security", "license", "update", "bump", "migrate", "upgrade", "docs", "status", "version",
        "init", "new",
    ] {
        let base = match command {
            "bump" => vec![command, "cargo:demo", "1.0.0"],
            "migrate" | "upgrade" => vec![command, "--from=1.0.0", "--to=2.0.0"],
            "new" => vec![command, "rust"],
            _ => vec![command],
        };
        for option in [
            "--fail-on=error",
            "--report=junit=report.xml",
            "--bazel",
            "--check",
        ] {
            let supported = match option {
                "--fail-on=error" => command == "security" || command == "license",
                "--report=junit=report.xml" => command == "security" || command == "license",
                "--check" => matches!(command, "generate" | "update" | "docs" | "version"),
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
        vec!["migrate", "--from="],
        vec!["migrate", "--to="],
        vec!["lint", "--pin=1.0.0"],
        vec!["build", "--pin=1.0.0"],
        vec!["clean", "--pin=1.0.0"],
        vec!["coverage", "--pin=1.0.0"],
        vec!["status", "extra"],
        vec!["version", "extra"],
        vec!["init", "one", "two"],
        vec!["watch"],
        vec!["docs", ":relative"],
        vec!["docs", ""],
        vec!["generate", ":relative"],
        vec!["security", ""],
        vec!["update", ""],
        vec!["migrate", "--from=1.0.0", "--to=2.0.0", ":relative"],
        vec!["--bazel", "bazel", "version"],
    ] {
        assert!(parse(&strings(&words)).is_err(), "{words:?}");
    }
}

const OWNED_OPTIONS: [(&str, &[&str], &[&str]); 9] = [
    (
        "--include-edits",
        &["--include-edits", "--output=json"],
        &["lint", "typecheck", "format", "check", "fix"],
    ),
    ("--configured", &["--configured"], &["list-tests"]),
    ("--cases", &["--cases"], &["list-tests"]),
    ("--min-coverage", &["--min-coverage=1"], &["coverage"]),
    ("--serve", &["--serve"], &["docs"]),
    ("--port", &["--serve", "--port=8080"], &["docs"]),
    ("--host", &["--serve", "--host=127.0.0.1"], &["docs"]),
    ("--open", &["--serve", "--open"], &["docs"]),
    ("--prune-unobserved", &["--prune-unobserved"], &["clean"]),
];

fn required_words(name: &str) -> &'static [&'static str] {
    match name {
        "bump" => &["cargo:demo", "1.0.0"],
        "migrate" | "upgrade" => &["--from=1.0.0", "--to=2.0.0"],
        "run" | "deploy" => &["//:demo"],
        "new" => &["rust"],
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
    for command in ["lint", "typecheck", "format", "check", "fix"] {
        for output in [None, Some("--output=text"), Some("--output=diff")] {
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

const REGISTRY_GATES: [(&str, fn(Command) -> bool, &[&str]); 7] = [
    ("--check", Command::supports_check, &["--check"]),
    ("--fail-on", Command::supports_fail_on, &["--fail-on=info"]),
    (
        "--fail-on",
        Command::supports_fail_on,
        &["--fail-on=warning"],
    ),
    ("--fail-on", Command::supports_fail_on, &["--fail-on=error"]),
    ("--debug", Command::supports_profile, &["--debug"]),
    ("--release", Command::supports_profile, &["--release"]),
    (
        "--min-coverage",
        Command::supports_min_coverage,
        &["--min-coverage=80"],
    ),
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

const OUTPUT_MODES: [(&str, &str, fn(Command) -> bool); 2] = [
    ("--output=json", "json", Command::supports_json),
    ("--output=diff", "diff", Command::supports_diff),
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
fn a_profile_flag_is_advertised_exactly_where_the_registry_declares_it() {
    for command in Command::value_variants() {
        assert_eq!(
            command.flags().contains("--debug | --release"),
            command.supports_profile(),
            "dx {} help must match supports_profile",
            command.name()
        );
    }
}

fn usage_banner() -> String {
    super::super::help::usage_banner()
}

fn banner_flags(clause: &str) -> Vec<String> {
    let chars: Vec<char> = clause.chars().collect();
    let mut flags = Vec::new();
    let mut index = 0;
    while index + 2 < chars.len() {
        if chars[index] == '-' && chars[index + 1] == '-' {
            let mut end = index + 2;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '-') {
                end += 1;
            }
            if end > index + 2 {
                flags.push(chars[index..end].iter().collect());
            }
            index = end;
            continue;
        }
        index += 1;
    }
    flags
}

fn banner_flag_claims() -> Vec<(String, Vec<String>)> {
    let text = usage_banner();
    let section = text
        .split_once("per-command flags: ")
        .expect("the usage banner names the per-command flags")
        .1;
    let mut claims: Vec<(String, Vec<String>)> = Vec::new();
    for clause in section.split(';') {
        let clause = clause.trim();
        let Some(start) = clause.find("--") else {
            continue;
        };
        let names: Vec<String> = clause[..start]
            .trim()
            .split('|')
            .map(str::to_owned)
            .collect();
        if !names
            .iter()
            .all(|name| Command::value_variants().iter().any(|it| it.name() == name))
        {
            continue;
        }
        for flag in banner_flags(&clause[start..]) {
            match claims.iter_mut().find(|(claimed, _)| claimed == &flag) {
                Some((_, owners)) => owners.extend(names.iter().cloned()),
                None => claims.push((flag, names.clone())),
            }
        }
    }
    for (_, owners) in &mut claims {
        owners.sort();
        owners.dedup();
    }
    claims.sort();
    claims
}

fn commands_supporting(supported: fn(Command) -> bool) -> Vec<String> {
    let mut names: Vec<String> = Command::value_variants()
        .iter()
        .filter(|command| supported(**command))
        .map(|command| command.name().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_usage_banner_claims_exactly_the_flags_the_registry_gates() {
    let profile = commands_supporting(Command::supports_profile);
    let mut expected: Vec<(String, Vec<String>)> = OWNED_OPTIONS
        .iter()
        .map(|(option, _, owners)| {
            let mut owners: Vec<String> = owners.iter().map(|name| (*name).to_owned()).collect();
            owners.sort();
            ((*option).to_owned(), owners)
        })
        .collect();
    expected.push(("--debug".to_owned(), profile.clone()));
    expected.push(("--release".to_owned(), profile));
    expected.push((
        "--check".to_owned(),
        vec!["docs".to_owned(), "version".to_owned()],
    ));
    expected.sort();
    assert_eq!(
        banner_flag_claims(),
        expected,
        "the usage banner must claim every gated per-command flag for exactly its owners:\n{}",
        usage_banner()
    );
}

#[test]
fn the_usage_banner_check_claim_matches_the_registry() {
    let text = usage_banner();
    let rest = text
        .split_once("--check is per-command only (")
        .expect("the usage banner scopes --check to a command list")
        .1;
    let list = rest
        .split(';')
        .next()
        .expect("the --check claim ends its command list")
        .trim();
    let mut claimed: Vec<String> = Vec::new();
    for name in list.split(['/', '|']) {
        if name == "quality" {
            claimed.extend(["lint", "typecheck", "format", "generate"].map(str::to_owned));
        } else {
            claimed.push(name.to_owned());
        }
    }
    claimed.sort();
    assert_eq!(
        claimed,
        commands_supporting(Command::supports_check),
        "the usage banner --check list must name every command that takes it:\n{text}"
    );
    assert!(
        text.contains("status rejects --check"),
        "the usage banner must keep naming status as a --check holdout:\n{text}"
    );
    assert!(
        !Command::Status.supports_check(),
        "the usage banner says status rejects --check"
    );
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
fn file_defaults_load_from_workspace_and_reject_invalid_toml() {
    let scratch = dx_test_scratch::scratch("parser-file-defaults-");
    assert_eq!(
        super::load_file_defaults(scratch.path()).expect("absent"),
        super::super::FileDefaults::default()
    );
    std::fs::write(scratch.path().join("MODULE.bazel"), "").expect("workspace");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join("dx.toml"), "[dx]\nquiet = true\n").expect("defaults");
    assert_eq!(
        super::load_file_defaults(scratch.path())
            .expect("defaults")
            .quiet,
        Some(true)
    );
    std::fs::write(scratch.path().join("dx.toml"), "[broken").expect("bad defaults");
    assert!(super::load_file_defaults(scratch.path()).is_err());
    std::fs::write(scratch.path().join("dx.toml"), "[dx]\nqiet = true\n").expect("misspelled key");
    let detail = super::load_file_defaults(scratch.path()).expect_err("a typo is not ignored");
    assert!(
        detail.contains("unknown field `qiet`"),
        "the diagnostic names the key: {detail}"
    );
    assert!(
        detail.contains("dx.toml"),
        "the diagnostic names the file: {detail}"
    );
}

#[test]
fn bump_needs_exactly_one_selector_plus_version() {
    let bump = parse(&strings(&["bump", "cargo:anyhow", "1.2.3"])).expect("parse bump");
    assert_eq!(bump.command, Command::Bump);
    assert_eq!(bump.command.name(), "bump");
    assert!(bump.command.is_audit_update());
    assert!(!bump.command.is_adoption());
    assert!(!bump.command.is_managed());
    assert_eq!(
        bump.targets,
        vec!["cargo:anyhow".to_owned(), "1.2.3".to_owned()]
    );
    assert_eq!(
        parse(&strings(&["bump"])),
        Err(ArgsError::MissingValue {
            option: "<set:package> <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["bump", "cargo:anyhow"])),
        Err(ArgsError::MissingValue {
            option: "<set:package> <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["bump", "cargo:anyhow", "1.2.3", "npm:react"])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "npm:react".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["bump", "cargo:anyhow", "1.2.3", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "bump",
            "cargo:anyhow",
            "1.2.3",
            "--fail-on=error"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--fail-on".to_owned(),
        })
    );
    let got = parse(&strings(&[
        "bump",
        "cargo:anyhow",
        "1.2.3",
        "--output=json",
    ]))
    .expect("bump json");
    assert_eq!(got.command, Command::Bump);
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(
        parse(&strings(&[
            "bump",
            "cargo:anyhow",
            "1.2.3",
            "--output=diff"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "bump",
            "cargo:anyhow",
            "1.2.3",
            "--report=sarif=out.sarif"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--report=sarif=out.sarif".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "bump",
            "cargo:anyhow",
            "1.2.3",
            "--",
            "--jobs=4"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--".to_owned(),
        })
    );
}

#[test]
fn migrate_needs_from_and_to_versions() {
    let migrate =
        parse(&strings(&["migrate", "--from=1.2.3", "--to=2.0.0"])).expect("parse migrate");
    assert_eq!(migrate.command, Command::Migrate);
    assert_eq!(migrate.command.name(), "migrate");
    assert!(!migrate.command.is_audit_update());
    assert!(!migrate.command.is_adoption());
    assert!(!migrate.command.is_managed());
    assert!(!migrate.command.is_mutating_by_default());
    assert_eq!(migrate.from, Some("1.2.3".to_owned()));
    assert_eq!(migrate.to, Some("2.0.0".to_owned()));
    let spaced =
        parse(&strings(&["migrate", "--from", "1.2.3", "--to", "2.0.0"])).expect("spaced parse");
    assert_eq!(spaced.from, Some("1.2.3".to_owned()));
    assert_eq!(spaced.to, Some("2.0.0".to_owned()));
    let scoped = parse(&strings(&[
        "migrate",
        "--from=1.2.3",
        "--to=2.0.0",
        "//a:one",
    ]))
    .expect("scoped parse");
    assert_eq!(scoped.targets, vec!["//a:one".to_owned()]);
    assert_eq!(
        parse(&strings(&["migrate", "--from=1.2.3"])),
        Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["migrate"])),
        Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        })
    );
    assert_usage(
        &["migrate", "--from", "--to=2.0.0"],
        parse(&strings(&["migrate", "--from", "--to=2.0.0"])).unwrap_err(),
        &["--from"],
    );
    assert_eq!(
        parse(&strings(&[
            "migrate",
            "--from=1.2.3",
            "--to=2.0.0",
            "--check"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "migrate",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "migrate",
            "--from=1.2.3",
            "--to=2.0.0",
            "--fail-on=error"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "migrate",
            option: "--fail-on".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "migrate",
            "--from=1.2.3",
            "--to=2.0.0",
            "--report=sarif=out.sarif"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "migrate",
            option: "--report=sarif=out.sarif".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "migrate",
            "--from=1.2.3",
            "--to=2.0.0",
            "--",
            "--jobs=4"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "migrate",
            option: "--".to_owned(),
        })
    );
    let upgrade_ok =
        parse(&strings(&["upgrade", "--from=1.2.3", "--to=2.0.0"])).expect("upgrade parses");
    assert_eq!(upgrade_ok.command, Command::Upgrade);
    assert_eq!(upgrade_ok.from, Some("1.2.3".to_owned()));
    assert_eq!(upgrade_ok.to, Some("2.0.0".to_owned()));
    assert_eq!(
        parse(&strings(&["lint", "--from=1.0.0"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--from".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["build", "//a:one", "--to=2.0.0"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--to".to_owned(),
        })
    );
}

#[test]
fn new_takes_language_plus_optional_name() {
    let got = parse(&strings(&["new", "rust", "demo"])).expect("parse new");
    assert_eq!(got.command, Command::New);
    assert!(got.command.is_adoption());
    assert!(!got.command.is_mutating_by_default());
    assert!(!got.command.supports_json());
    assert_eq!(got.targets, vec!["rust".to_owned(), "demo".to_owned()]);
    let bare_lang = parse(&strings(&["new", "go"])).expect("language only");
    assert_eq!(bare_lang.targets, vec!["go".to_owned()]);
    assert_eq!(
        parse(&strings(&["new"])),
        Err(ArgsError::MissingValue {
            option: "<language>".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["new", "rust", "a", "b"])),
        Err(ArgsError::UnsupportedOption {
            command: "new",
            option: "b".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&["new", "rust", "--output=json"])),
        Err(ArgsError::UnsupportedOption {
            command: "new",
            option: "--output=json".to_owned(),
        })
    );
}

#[test]
fn upgrade_needs_from_and_to_with_no_scopes() {
    let got = parse(&strings(&["upgrade", "--from=1.2.3", "--to=2.0.0"])).expect("parse upgrade");
    assert_eq!(got.command, Command::Upgrade);
    assert!(got.command.is_adoption());
    assert!(!got.command.is_mutating_by_default());
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&strings(&["upgrade", "--from=1.2.3"])),
        Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "upgrade",
            "--from=1.2.3",
            "--to=2.0.0",
            "//a:one"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "upgrade",
            option: "//a:one".to_owned(),
        })
    );
    assert_eq!(
        parse(&strings(&[
            "upgrade",
            "--from=1.2.3",
            "--to=2.0.0",
            "--output=diff"
        ])),
        Err(ArgsError::UnsupportedOption {
            command: "upgrade",
            option: "--output=diff".to_owned(),
        })
    );
    let json = parse(&strings(&[
        "upgrade",
        "--from=1.2.3",
        "--to=2.0.0",
        "--output=json",
    ]))
    .expect("upgrade json");
    assert_eq!(json.output, OutputMode::Json);
}

#[test]
fn run_rejects_machine_output_and_reports() {
    let got = parse(&strings(&["run", "//app:bin", "--output=json"])).expect("run json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&strings(&["run", "//app:bin", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "run",
            option: "--output=diff".to_owned(),
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
    assert_eq!(got.bazel_options, vec!["--port=8080".to_owned()]);
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
        "fix",
        "security",
        "license",
        "update",
        "clean",
        "codegen",
        "env",
        "docs",
    ] {
        let got = parse(&strings(&[command, "--output=json"])).expect("json capable");
        assert_eq!(got.output, OutputMode::Json, "command: {command}");
        assert!(got.command.supports_json(), "command: {command}");
    }
    let got = parse(&strings(&[
        "bump",
        "cargo:anyhow",
        "1.2.3",
        "--output=json",
    ]))
    .expect("bump json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&strings(&[
        "migrate",
        "--from=1.2.3",
        "--to=2.0.0",
        "--output=json",
    ]))
    .expect("migrate json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&strings(&[
        "upgrade",
        "--from=1.2.3",
        "--to=2.0.0",
        "--output=json",
    ]))
    .expect("upgrade json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&strings(&["status", "--output=json"])).expect("status json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Status.supports_json());
    let got = parse(&strings(&["version", "--output=json"])).expect("version json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Version.supports_json());
    for (words, want) in [
        (vec!["watch", "test", "--output=json"], OutputMode::Json),
        (vec!["watch", "build", "-o", "json"], OutputMode::Json),
        (vec!["watch", "lint", "--output=diff"], OutputMode::Diff),
        (vec!["watch", "fix", "-o", "diff"], OutputMode::Diff),
        (vec!["watch", "-o", "diff", "format"], OutputMode::Diff),
    ] {
        let got = parse(&strings(&words)).expect("the wrapped command takes the mode");
        assert_eq!(got.command, Command::Watch, "{words:?}");
        assert_eq!(got.output, want, "{words:?}");
    }
    for command in ["lint", "typecheck", "format", "generate", "check", "fix"] {
        let got = parse(&strings(&[command, "--output=diff"])).expect("diff capable");
        assert_eq!(got.output, OutputMode::Diff, "command: {command}");
        assert!(got.command.supports_diff(), "command: {command}");
    }
    for words in [
        vec!["clean", "--output=diff"],
        vec!["codegen", "--output=diff"],
        vec!["env", "--output=diff"],
        vec!["run", "//app:bin", "--output=diff"],
        vec!["init", "--output=json"],
        vec!["init", "proj", "--output=diff"],
        vec!["new", "rust", "--output=json"],
        vec!["new", "rust", "demo", "--output=diff"],
        vec!["watch", "build", "--output=diff"],
        vec!["watch", "run", "//app:bin", "-o", "diff"],
        vec!["version", "--output=diff"],
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
        vec!["build", "//a:one", "--output=diff"],
        vec!["test", "//a:one", "--output=diff"],
        vec!["coverage", "--output=diff"],
        vec!["run", "//a:one", "--output=diff"],
        vec!["security", "--output=diff"],
        vec!["license", "--output=diff"],
        vec!["update", "--output=diff"],
        vec!["status", "--output=diff"],
        vec!["version", "--output=diff"],
        vec!["clean", "--output=diff"],
        vec!["codegen", "--output=diff"],
        vec!["env", "--output=diff"],
        vec!["docs", "--output=diff"],
        vec!["bump", "cargo:anyhow", "1.2.3", "--output=diff"],
        vec!["migrate", "--from=1.2.3", "--to=2.0.0", "--output=diff"],
        vec!["upgrade", "--from=1.2.3", "--to=2.0.0", "--output=diff"],
    ] {
        assert_eq!(
            parse(&strings(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--output=diff".to_owned(),
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
        vec!["--output=json", "bazel", "version"],
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
fn version_rollback_check_and_configured_parse() {
    let got = parse(&strings(&["version", "--check"])).expect("parse");
    assert!(got.check);
    let got = parse(&strings(&["list-tests", "--configured", "//a:one"])).expect("parse");
    assert_eq!(got.command, Command::ListTests);
    assert!(got.configured);
    for words in [
        vec!["status", "--serve"],
        vec!["status", "--check"],
        vec!["list-tests", "//a:one", "--from=0.1.0"],
        vec!["build", "//a:one", "--configured"],
        vec!["lint", "--serve"],
        vec!["check", "//...", "--configured"],
        vec!["status", "--from=0.1.0"],
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
fn check_rejects_apply_while_quality_defaults_to_check() {
    assert!(
        matches!(
            parse(&strings(&["check", "--apply"])),
            Err(ArgsError::UnsupportedOption { .. })
        ),
        "dx check stays read-only"
    );
    for words in [
        vec!["lint", "--check", "--apply"],
        vec!["fix", "--check", "--apply"],
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
    for command in ["lint", "typecheck", "format", "fix"] {
        let bare = parse(&strings(&[command])).expect("bare parses");
        assert!(!bare.check, "{command}");
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
    assert!(!got.check);
    assert!(!got.serve);
    assert_eq!(got.port, None);
    assert_eq!(got.host, None);
    assert!(!got.open);
    let got = parse(&strings(&["docs", "--check"])).expect("check parses");
    assert!(got.check);
    let got = parse(&strings(&["docs", "--serve"])).expect("serve parses");
    assert!(got.serve);
    assert_eq!(got.port, None);
    let got = parse(&strings(&["docs", "--serve", "--port=8080"])).expect("port parses");
    assert!(got.serve);
    assert_eq!(got.port, Some(8080));
    let got =
        parse(&strings(&["docs", "--check", "--serve", "--port", "9000"])).expect("split port");
    assert!(got.check);
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
        vec!["docs", "--from=0.1.0"],
        vec!["docs", "--output=diff"],
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
    let got =
        parse(&strings(&["docs", "--check", "--", "--config=ci"])).expect("docs check forwards");
    assert!(got.check);
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
        &["lint", "--color"],
        parse(&strings(&["lint", "--color"])).unwrap_err(),
        &["--color"],
    );
}

#[test]
fn managed_commands_parse_repo_and_exact_scopes() {
    for command in ["codegen", "env"] {
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
        parse(&strings(&["codegen", "//a:one", "//b:two"])),
        Err(ArgsError::UnsupportedOption {
            command: "codegen",
            option: "//b:two".to_owned(),
        })
    );
    for words in [
        vec!["env", "//a/..."],
        vec!["codegen", ":target"],
        vec!["env", ""],
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
        vec!["env", "--fail-on=error"],
        vec!["codegen", "--report=sarif=out.sarif"],
        vec!["env", "--from=0.1.0"],
        vec!["codegen", "--configured"],
        vec!["env", "--configured"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?}"
        );
    }
    for command in ["codegen", "env"] {
        let got = parse(&strings(&[command, "--check"])).expect("check parses");
        assert!(got.check, "{command} --check sets the flag");
        assert_eq!(got.operation(), OperationMode::Check);
        assert!(!got.applies());
        let got = parse(&strings(&[command, "--apply"])).expect("apply parses");
        assert!(got.applies());
        assert_eq!(
            parse(&strings(&[command, "--check", "--apply"])),
            Err(ArgsError::ConflictingModes {
                first: "--check",
                second: "--apply",
            }),
            "{command} --check with --apply conflicts"
        );
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
        "fix",
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
        vec!["bump", "cargo:anyhow", "1.2.3", "--here"],
        vec!["migrate", "--from=1.2.3", "--to=2.0.0", "--here"],
        vec!["codegen", "--here"],
        vec!["env", "--here"],
        vec!["run", "//app:bin", "--here"],
        vec!["deploy", "//app:bin", "--here"],
        vec!["status", "--here"],
        vec!["version", "--here"],
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
    assert!(!plain.check);
    assert_eq!(plain.targets, vec!["bash".to_owned()]);
    for words in [
        vec!["completion", "bash", "--check"],
        vec!["completion", "--check"],
    ] {
        assert!(
            matches!(
                parse(&strings(&words)),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "words: {words:?}"
        );
    }
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
    assert!(matches!(
        parse(&strings(&["status", "--check"])),
        Err(ArgsError::UnsupportedOption { .. })
    ));
}

#[test]
fn offline_and_frozen_are_independent_policies_on_audit_update_bump() {
    for command in ["security", "license", "update"] {
        let got = parse(&strings(&[command, "--offline"])).expect("offline parses");
        assert!(got.offline, "command: {command}");
        assert!(!got.frozen, "command: {command}");
        assert!(got.command.supports_offline(), "command: {command}");
        let frozen = parse(&strings(&[command, "--frozen"])).expect("frozen parses");
        assert!(frozen.frozen, "command: {command}");
        assert!(!frozen.offline, "command: {command}");
        assert!(frozen.command.supports_frozen(), "command: {command}");
        let both = parse(&strings(&[command, "--offline", "--frozen"])).expect("combined parses");
        assert!(both.offline && both.frozen, "command: {command}");
        let bare = parse(&strings(&[command])).expect("bare parses");
        assert!(!bare.offline, "command: {command}");
        assert!(!bare.frozen, "command: {command}");
    }
    let bump =
        parse(&strings(&["bump", "cargo:anyhow", "1.2.3", "--offline"])).expect("bump offline");
    assert!(bump.offline);
    assert!(!bump.frozen);
    assert!(bump.command.supports_offline());
    let bump_frozen =
        parse(&strings(&["bump", "cargo:anyhow", "1.2.3", "--frozen"])).expect("bump frozen");
    assert!(bump_frozen.frozen);
    assert!(!bump_frozen.offline);
    assert!(bump_frozen.command.supports_frozen());
    for words in [
        vec!["lint", "--offline"],
        vec!["build", "//a:one", "--offline"],
        vec!["clean", "--offline"],
        vec!["codegen", "--offline"],
        vec!["status", "--offline"],
        vec!["docs", "--offline"],
        vec!["migrate", "--from=1.2.3", "--to=2.0.0", "--offline"],
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
        &["security", "--offline=yes"],
        parse(&strings(&["security", "--offline=yes"])).unwrap_err(),
        &["--offline"],
    );
}

#[test]
fn a_refused_output_diff_is_named_before_any_other_flag_the_command_refuses() {
    for (command, also_refused) in [
        ("clean", "--report=junit=one.xml"),
        ("update", "--report=junit=one.xml"),
        ("bump", "--report=junit=one.xml"),
        ("migrate", "--report=junit=one.xml"),
        ("upgrade", "--report=junit=one.xml"),
        ("codegen", "--report=junit=one.xml"),
        ("env", "--report=junit=one.xml"),
        ("docs", "--port=1"),
    ] {
        let words = owned_option_words(
            Command::parse(command).unwrap_or_else(|| panic!("registry names {command}")),
            &["--output=diff", also_refused],
        );
        assert_eq!(
            parse(&words),
            Err(ArgsError::UnsupportedOption {
                command,
                option: "--output=diff".to_owned(),
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
            vec!["upgrade", "--from=1.0.0", "--to=2.0.0", "--", "--config=ci"],
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
    let got = parse(&strings(&["worktree", "add", "topic", "--output=path"])).expect("path");
    assert!(got.path_only);
    let got = parse(&strings(&["worktree", "release", "--force"])).expect("release");
    assert!(got.force);
    let got = parse(&strings(&["worktree", "prune", "--apply"])).expect("prune");
    assert!(got.apply);
    assert!(parse(&strings(&["worktree"])).is_ok());
    assert!(parse(&strings(&["worktree", "hook", "guard"])).is_ok());
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
    ] {
        assert!(parse(&strings(&words)).is_err(), "words: {words:?}");
    }
}
