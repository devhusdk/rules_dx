use super::super::{ArgsError, Command};
use super::parse;
use dx_output::OutputMode;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

#[test]
fn command_option_ownership_rejects_every_unsupported_surface() {
    for command in [
        "security", "license", "update", "bump", "migrate", "upgrade", "docs", "status", "version",
        "owners", "deps", "hooks", "init", "new",
    ] {
        let base = match command {
            "bump" => vec![command, "cargo:demo", "1.0.0"],
            "migrate" | "upgrade" => vec![command, "--from=1.0.0", "--to=2.0.0"],
            "owners" | "deps" => vec![command, "//:demo"],
            "hooks" => vec![command, "status"],
            "new" => vec![command, "rust"],
            _ => vec![command],
        };
        for option in [
            "--pin=1.0.0",
            "--fail-on=error",
            "--report=junit=report.xml",
            "--bazel",
            "--check",
        ] {
            let supported = match option {
                "--pin=1.0.0" => command == "version",
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
            assert!(parse(&args(&words)).is_err(), "{words:?}");
        }
        if !matches!(command, "generate" | "docs") {
            let mut words = base.clone();
            words.extend(["--", "--keep_going"]);
            assert!(parse(&args(&words)).is_err(), "{words:?}");
        }
    }
    for words in [
        vec!["version", "--pin="],
        vec!["migrate", "--from="],
        vec!["migrate", "--to="],
        vec!["status", "extra"],
        vec!["version", "extra"],
        vec!["init", "one", "two"],
        vec!["hooks"],
        vec!["watch"],
        vec!["owners"],
        vec!["deps"],
        vec!["docs", ":relative"],
        vec!["docs", ""],
        vec!["generate", ":relative"],
        vec!["security", ""],
        vec!["update", ""],
        vec!["migrate", "--from=1.0.0", "--to=2.0.0", ":relative"],
        vec!["--bazel", "bazel", "version"],
    ] {
        assert!(parse(&args(&words)).is_err(), "{words:?}");
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
    std::fs::write(
        scratch.path().join(".dx/config.toml"),
        "[dx]\nquiet = true\n",
    )
    .expect("defaults");
    assert_eq!(
        super::load_file_defaults(scratch.path())
            .expect("defaults")
            .quiet,
        Some(true)
    );
    std::fs::write(scratch.path().join(".dx/config.toml"), "[broken").expect("bad defaults");
    assert!(super::load_file_defaults(scratch.path()).is_err());
}

#[test]
fn bump_needs_exactly_one_selector_plus_version() {
    let bump = parse(&args(&["bump", "cargo:anyhow", "1.2.3"])).expect("parse bump");
    assert_eq!(bump.command, Command::Bump);
    assert_eq!(bump.command.name(), "bump");
    assert!(bump.command.is_audit_update());
    assert!(!bump.command.is_workflow());
    assert!(!bump.command.is_adoption());
    assert!(!bump.command.is_managed());
    assert_eq!(
        bump.targets,
        vec!["cargo:anyhow".to_owned(), "1.2.3".to_owned()]
    );
    assert_eq!(
        parse(&args(&["bump"])),
        Err(ArgsError::MissingValue {
            option: "<selector> <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["bump", "cargo:anyhow"])),
        Err(ArgsError::MissingValue {
            option: "<selector> <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["bump", "cargo:anyhow", "1.2.3", "npm:react"])),
        Err(ArgsError::MissingValue {
            option: "<selector> <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--fail-on=error"])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--fail-on".to_owned(),
        })
    );
    let got = parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--output=json"])).expect("bump json");
    assert_eq!(got.command, Command::Bump);
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(
        parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&[
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
        parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--", "--jobs=4"])),
        Err(ArgsError::UnsupportedOption {
            command: "bump",
            option: "--".to_owned(),
        })
    );
}

#[test]
fn migrate_needs_from_and_to_versions() {
    let migrate = parse(&args(&["migrate", "--from=1.2.3", "--to=2.0.0"])).expect("parse migrate");
    assert_eq!(migrate.command, Command::Migrate);
    assert_eq!(migrate.command.name(), "migrate");
    assert!(!migrate.command.is_audit_update());
    assert!(!migrate.command.is_workflow());
    assert!(!migrate.command.is_adoption());
    assert!(!migrate.command.is_managed());
    assert!(migrate.command.is_mutating_by_default());
    assert_eq!(migrate.from, Some("1.2.3".to_owned()));
    assert_eq!(migrate.to, Some("2.0.0".to_owned()));
    let spaced =
        parse(&args(&["migrate", "--from", "1.2.3", "--to", "2.0.0"])).expect("spaced parse");
    assert_eq!(spaced.from, Some("1.2.3".to_owned()));
    assert_eq!(spaced.to, Some("2.0.0".to_owned()));
    let scoped =
        parse(&args(&["migrate", "--from=1.2.3", "--to=2.0.0", "//a:one"])).expect("scoped parse");
    assert_eq!(scoped.targets, vec!["//a:one".to_owned()]);
    assert_eq!(
        parse(&args(&["migrate", "--from=1.2.3"])),
        Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["migrate"])),
        Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["migrate", "--from", "--to=2.0.0"])),
        Err(ArgsError::MissingValue {
            option: "--from".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["migrate", "--from=1.2.3", "--to=2.0.0", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "migrate",
            option: "--check".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&[
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
        parse(&args(&[
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
        parse(&args(&[
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
        parse(&args(&["upgrade", "--from=1.2.3", "--to=2.0.0"])).expect("upgrade parses");
    assert_eq!(upgrade_ok.command, Command::Upgrade);
    assert_eq!(upgrade_ok.from, Some("1.2.3".to_owned()));
    assert_eq!(upgrade_ok.to, Some("2.0.0".to_owned()));
    assert_eq!(
        parse(&args(&["lint", "--from=1.0.0"])),
        Err(ArgsError::UnsupportedOption {
            command: "lint",
            option: "--from".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["build", "//a:one", "--to=2.0.0"])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--to".to_owned(),
        })
    );
}

#[test]
fn new_takes_language_plus_optional_name() {
    let got = parse(&args(&["new", "rust", "demo"])).expect("parse new");
    assert_eq!(got.command, Command::New);
    assert!(got.command.is_adoption());
    assert!(got.command.is_mutating_by_default());
    assert!(!got.command.supports_json());
    assert_eq!(got.targets, vec!["rust".to_owned(), "demo".to_owned()]);
    let bare_lang = parse(&args(&["new", "go"])).expect("language only");
    assert_eq!(bare_lang.targets, vec!["go".to_owned()]);
    assert_eq!(
        parse(&args(&["new"])),
        Err(ArgsError::MissingValue {
            option: "<language> [name]".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["new", "rust", "a", "b"])),
        Err(ArgsError::MissingValue {
            option: "<language> [name]".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["new", "rust", "--output=json"])),
        Err(ArgsError::UnsupportedOption {
            command: "new",
            option: "--output=json".to_owned(),
        })
    );
}

#[test]
fn upgrade_needs_from_and_to_with_no_scopes() {
    let got = parse(&args(&["upgrade", "--from=1.2.3", "--to=2.0.0"])).expect("parse upgrade");
    assert_eq!(got.command, Command::Upgrade);
    assert!(got.command.is_adoption());
    assert!(got.command.is_mutating_by_default());
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&args(&["upgrade", "--from=1.2.3"])),
        Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["upgrade", "--from=1.2.3", "--to=2.0.0", "//a:one"])),
        Err(ArgsError::UnsupportedOption {
            command: "upgrade",
            option: "//a:one".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&[
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
    let json = parse(&args(&[
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
    let got = parse(&args(&["run", "//app:bin", "--output=json"])).expect("run json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    assert_eq!(
        parse(&args(&["run", "//app:bin", "--output=diff"])),
        Err(ArgsError::UnsupportedOption {
            command: "run",
            option: "--output=diff".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["run", "--report=junit=out.xml"])),
        Err(ArgsError::UnsupportedOption {
            command: "run",
            option: "--report=junit=out.xml".to_owned(),
        })
    );
    let got = parse(&args(&["run", "//app:bin", "--", "--port=8080"])).expect("parse run");
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
        "setup",
        "docs",
    ] {
        let got = parse(&args(&[command, "--output=json"])).expect("json capable");
        assert_eq!(got.output, OutputMode::Json, "command: {command}");
        assert!(got.command.supports_json(), "command: {command}");
    }
    let got = parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--output=json"])).expect("bump json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&args(&[
        "migrate",
        "--from=1.2.3",
        "--to=2.0.0",
        "--output=json",
    ]))
    .expect("migrate json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&args(&[
        "upgrade",
        "--from=1.2.3",
        "--to=2.0.0",
        "--output=json",
    ]))
    .expect("upgrade json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.command.supports_json());
    let got = parse(&args(&["status", "--output=json"])).expect("status json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Status.supports_json());
    let got = parse(&args(&["version", "--output=json"])).expect("version json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Version.supports_json());
    let got = parse(&args(&["owners", "//a:one", "--output=json"])).expect("owners json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Owners.supports_json());
    let got = parse(&args(&["deps", "//a:one", "--output=json"])).expect("deps json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Deps.supports_json());
    let got = parse(&args(&["why", "src/main.rs", "//a:one", "--output=json"])).expect("why json");
    assert_eq!(got.output, OutputMode::Json);
    assert!(Command::Why.supports_json());
    for command in ["lint", "typecheck", "format", "generate", "check", "fix"] {
        let got = parse(&args(&[command, "--output=diff"])).expect("diff capable");
        assert_eq!(got.output, OutputMode::Diff, "command: {command}");
        assert!(got.command.supports_diff(), "command: {command}");
    }
    for words in [
        vec!["clean", "--output=diff"],
        vec!["codegen", "--output=diff"],
        vec!["env", "--output=diff"],
        vec!["setup", "--output=diff"],
        vec!["run", "//app:bin", "--output=diff"],
        vec!["--output=json", "bazel", "version"],
        vec!["init", "--output=json"],
        vec!["init", "proj", "--output=diff"],
        vec!["new", "rust", "--output=json"],
        vec!["new", "rust", "demo", "--output=diff"],
        vec!["hooks", "status", "--output=json"],
        vec!["watch", "test", "--output=json"],
        vec!["version", "--output=diff"],
        vec!["owners", "//a:one", "--output=diff"],
        vec!["deps", "//a:one", "--output=diff"],
        vec!["why", "src/main.rs", "//a:one", "--output=diff"],
        vec!["completion", "bash", "--output=json"],
    ] {
        assert!(
            matches!(
                parse(&args(&words)),
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
        vec!["owners", "//a:one", "--output=diff"],
        vec!["deps", "//a:one", "--output=diff"],
        vec!["why", "src/main.rs", "//a:one", "--output=diff"],
        vec!["clean", "--output=diff"],
        vec!["codegen", "--output=diff"],
        vec!["env", "--output=diff"],
        vec!["setup", "--output=diff"],
        vec!["docs", "--output=diff"],
        vec!["bump", "cargo:anyhow", "1.2.3", "--output=diff"],
        vec!["migrate", "--from=1.2.3", "--to=2.0.0", "--output=diff"],
        vec!["upgrade", "--from=1.2.3", "--to=2.0.0", "--output=diff"],
    ] {
        assert_eq!(
            parse(&args(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--output=diff".to_owned(),
            }),
            "words: {words:?}"
        );
    }
}

#[test]
fn bazel_forwards_verbatim_and_rejects_dx_options() {
    let got = parse(&args(&["bazel", "build", "//...", "--", "--jobs=4"])).expect("parse");
    assert_eq!(got.command, Command::Bazel);
    assert_eq!(got.command.name(), "bazel");
    assert!(!got.command.is_workflow());
    assert!(!got.command.is_adoption());
    assert!(got.targets.is_empty());
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
        vec!["bazel", "version", "--configured"],
        vec!["bazel", "--", "--fail-on=error"],
    ] {
        let got = parse(&args(&words)).expect("verbatim");
        assert_eq!(got.command, Command::Bazel, "words: {words:?}");
        assert!(got.targets.is_empty(), "words: {words:?}");
    }
    let got = parse(&args(&["bazel", "build", "--jobs", "4"])).expect("verbatim");
    assert_eq!(
        got.bazel_options,
        vec!["build".to_owned(), "--jobs".to_owned(), "4".to_owned()]
    );
    for words in [
        vec!["--output=json", "bazel", "version"],
        vec!["--check", "bazel", "version"],
        vec!["--pin=0.1.0", "bazel", "version"],
        vec!["--configured", "bazel", "version"],
        vec!["--fail-on=error", "bazel", "build", "//..."],
        vec!["--report=sarif=x.sarif", "bazel", "build"],
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
fn version_rollback_check_and_configured_parse() {
    let got = parse(&args(&["version", "--rollback"])).expect("parse");
    assert_eq!(got.command, Command::Version);
    assert!(got.rollback);
    let got = parse(&args(&["version", "--check"])).expect("parse");
    assert!(got.check);
    let got = parse(&args(&["deps", "--configured", "//a:one"])).expect("parse");
    assert_eq!(got.command, Command::Deps);
    assert!(got.configured);
    for words in [
        vec!["status", "--rollback"],
        vec!["status", "--check"],
        vec!["owners", "//a:one", "--pin=0.1.0"],
        vec!["build", "//a:one", "--configured"],
        vec!["lint", "--rollback"],
        vec!["check", "//...", "--configured"],
        vec!["status", "--pin=0.1.0"],
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
fn docs_check_serve_port_parse() {
    let got = parse(&args(&["docs"])).expect("bare docs parses");
    assert_eq!(got.command, Command::Docs);
    assert!(!got.check);
    assert!(!got.serve);
    assert_eq!(got.port, None);
    assert_eq!(got.host, None);
    assert!(!got.open);
    let got = parse(&args(&["docs", "--check"])).expect("check parses");
    assert!(got.check);
    let got = parse(&args(&["docs", "--serve"])).expect("serve parses");
    assert!(got.serve);
    assert_eq!(got.port, None);
    let got = parse(&args(&["docs", "--serve", "--port=8080"])).expect("port parses");
    assert!(got.serve);
    assert_eq!(got.port, Some(8080));
    let got = parse(&args(&["docs", "--check", "--serve", "--port", "9000"])).expect("split port");
    assert!(got.check);
    assert!(got.serve);
    assert_eq!(got.port, Some(9000));
    let got = parse(&args(&["docs", "--serve", "--host=example.test"])).expect("host parses");
    assert_eq!(got.host, Some("example.test".to_owned()));
    let got = parse(&args(&["docs", "--serve", "--host", "example.test"])).expect("split host");
    assert_eq!(got.host, Some("example.test".to_owned()));
    let got = parse(&args(&["docs", "--serve", "--open"])).expect("open parses");
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
        vec!["docs", "--pin=0.1.0"],
        vec!["docs", "--output=diff"],
    ] {
        assert!(
            matches!(
                parse(&args(&words)),
                Err(ArgsError::UnsupportedOption { .. })
                    | Err(ArgsError::MissingValue { .. })
                    | Err(ArgsError::UnknownOption { .. })
            ),
            "words: {words:?}"
        );
    }
    let got = parse(&args(&["docs", "--", "--jobs=4"])).expect("docs forwards command options");
    assert_eq!(got.bazel_options, args(&["--jobs=4"]));
    let got = parse(&args(&["docs", "--check", "--", "--config=ci"])).expect("docs check forwards");
    assert!(got.check);
    assert_eq!(got.bazel_options, args(&["--config=ci"]));
}

#[test]
fn color_parses_globally_with_bad_values_rejected() {
    use dx_output::ColorMode;
    let got = parse(&args(&["lint"])).expect("default color");
    assert_eq!(got.color, ColorMode::Auto);
    let got = parse(&args(&["lint", "--color=never"])).expect("never parses");
    assert_eq!(got.color, ColorMode::Never);
    let got = parse(&args(&["--color=always", "lint"])).expect("global position");
    assert_eq!(got.color, ColorMode::Always);
    assert_eq!(
        parse(&args(&["lint", "--color=bright"])),
        Err(ArgsError::BadColor {
            value: "bright".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["lint", "--color"])),
        Err(ArgsError::MissingValue {
            option: "--color".to_owned(),
        })
    );
}

#[test]
fn managed_commands_parse_repo_and_exact_scopes() {
    for command in ["codegen", "env", "setup"] {
        let got = parse(&args(&[command])).expect("parse");
        assert!(got.command.is_managed());
        assert!(got.targets.is_empty());
        assert!(got.bazel_options.is_empty());
        let got = parse(&args(&[command, "//a:one", "--", "--jobs=4"])).expect("scoped parse");
        assert_eq!(got.targets, args(&["//a:one"]));
        assert_eq!(got.bazel_options, args(&["--jobs=4"]));
        let got = parse(&args(&[command, "@repo//pkg:lib"])).expect("external label");
        assert_eq!(got.targets, args(&["@repo//pkg:lib"]));
    }
    assert_eq!(
        parse(&args(&["codegen", "//a:one", "//b:two"])),
        Err(ArgsError::UnsupportedOption {
            command: "codegen",
            option: "//b:two".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["setup", "//a:one", "//b:two"])),
        Err(ArgsError::UnsupportedOption {
            command: "setup",
            option: "//b:two".to_owned(),
        })
    );
    for words in [
        vec!["env", "//a/..."],
        vec!["setup", "src/main.rs"],
        vec!["codegen", ":target"],
        vec!["env", ""],
        vec!["setup", "--config=release"],
    ] {
        assert!(
            matches!(
                parse(&args(&words)),
                Err(ArgsError::EmptyScope)
                    | Err(ArgsError::RelativeLabel { .. })
                    | Err(ArgsError::InvalidScope { .. })
                    | Err(ArgsError::UnknownOption { .. })
            ),
            "words: {words:?}"
        );
    }
    for words in [
        vec!["codegen", "--check"],
        vec!["env", "--fail-on=error"],
        vec!["setup", "--output=diff"],
        vec!["codegen", "--report=sarif=out.sarif"],
        vec!["env", "--pin=0.1.0"],
        vec!["setup", "--rollback"],
        vec!["codegen", "--configured"],
        vec!["env", "--configured"],
        vec!["setup", "--pin=0.2.0"],
        vec!["codegen", "--bazel"],
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
fn why_requires_file_and_label() {
    let got = parse(&args(&["why", "src/a.rs", "//a:one"])).expect("parse");
    assert_eq!(got.command, Command::Why);
    assert_eq!(
        got.targets,
        vec!["src/a.rs".to_owned(), "//a:one".to_owned()]
    );
    for words in [
        vec!["why", "src/a.rs"],
        vec!["why", "src/a.rs", "//a:one", "//b:two"],
    ] {
        assert_eq!(
            parse(&args(&words)),
            Err(ArgsError::MissingValue {
                option: "<file> <label>".to_owned(),
            }),
            "words: {words:?}"
        );
    }
}

#[test]
fn init_and_hooks_have_no_force_flag() {
    for words in [
        vec!["init", "--force"],
        vec!["init", "--force", "demo"],
        vec!["hooks", "install", "--force"],
    ] {
        assert!(
            matches!(
                parse(&args(&words)),
                Err(ArgsError::UnknownOption { option, .. }) if option == "--force"
            ),
            "words: {words:?} must reject --force as unknown, got {:?}",
            parse(&args(&words))
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
        let got = parse(&args(&[command, "--here"])).expect("here parses");
        assert!(got.here, "command: {command}");
        assert!(got.targets.is_empty(), "command: {command}");
        assert!(got.command.supports_here(), "command: {command}");
        let alias = parse(&args(&[command, "--cwd"])).expect("cwd alias parses");
        assert!(alias.here, "command: {command}");
        let before = parse(&args(&["--here", command])).expect("before parses");
        assert!(before.here, "command: {command}");
        let bare = parse(&args(&[command])).expect("bare parses");
        assert!(!bare.here, "command: {command}");
        assert!(bare.targets.is_empty(), "command: {command}");
    }
    for words in [
        vec!["build", "--here", "//a:one"],
        vec!["lint", "src/a.py", "--here"],
        vec!["test", "--cwd", "//..."],
        vec!["security", "--here", "//a:one"],
        vec!["license", "cli/cli", "--here"],
    ] {
        assert_eq!(
            parse(&args(&words)),
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
        vec!["setup", "--here"],
        vec!["run", "//app:bin", "--here"],
        vec!["deploy", "//app:bin", "--here"],
        vec!["status", "--here"],
        vec!["version", "--here"],
        vec!["owners", "//a:one", "--here"],
        vec!["completion", "bash", "--here"],
    ] {
        assert_eq!(
            parse(&args(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--here".to_owned(),
            }),
            "words: {words:?}"
        );
    }
    let verbatim = parse(&args(&["bazel", "build", "--here"])).expect("verbatim");
    assert_eq!(verbatim.command, Command::Bazel);
    assert!(!verbatim.here);
    assert_eq!(
        parse(&args(&["--here", "bazel", "version"])),
        Err(ArgsError::UnsupportedOption {
            command: "bazel",
            option: "--here".to_owned(),
        })
    );
}

#[test]
fn completion_check_verifies_without_writing() {
    let one = parse(&args(&["completion", "bash", "--check"])).expect("one shell check");
    assert_eq!(one.command, Command::Completion);
    assert!(one.check);
    assert_eq!(one.targets, vec!["bash".to_owned()]);
    let all = parse(&args(&["completion", "--check"])).expect("all shells check");
    assert_eq!(all.command, Command::Completion);
    assert!(all.check);
    assert!(all.targets.is_empty());
    let plain = parse(&args(&["completion", "bash"])).expect("plain renders");
    assert!(!plain.check);
    assert_eq!(
        parse(&args(&["completion", "bash", "zsh", "--check"])),
        Err(ArgsError::UnsupportedOption {
            command: "completion",
            option: "zsh".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["completion"])),
        Err(ArgsError::MissingValue {
            option: "<shell>".to_owned(),
        })
    );
    assert!(matches!(
        parse(&args(&["status", "--check"])),
        Err(ArgsError::UnsupportedOption { .. })
    ));
}

#[test]
fn offline_forces_cache_only_on_audit_update_bump() {
    for command in ["security", "license", "update"] {
        let got = parse(&args(&[command, "--offline"])).expect("offline parses");
        assert!(got.offline, "command: {command}");
        assert!(got.command.supports_offline(), "command: {command}");
        let alias = parse(&args(&[command, "--frozen"])).expect("frozen alias parses");
        assert!(alias.offline, "command: {command}");
        let before = parse(&args(&["--offline", command])).expect("before parses");
        assert!(before.offline, "command: {command}");
        let bare = parse(&args(&[command])).expect("bare parses");
        assert!(!bare.offline, "command: {command}");
    }
    let bump = parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--offline"])).expect("bump offline");
    assert!(bump.offline);
    assert!(bump.command.supports_offline());
    let bump_alias =
        parse(&args(&["bump", "cargo:anyhow", "1.2.3", "--frozen"])).expect("bump frozen");
    assert!(bump_alias.offline);
    let dry = parse(&args(&["update", "--offline", "--dry-run"])).expect("offline dry-run");
    assert!(dry.offline);
    assert!(dry.dry_run);
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
            parse(&args(&words)),
            Err(ArgsError::UnsupportedOption {
                command: words[0],
                option: "--offline".to_owned(),
            }),
            "words: {words:?}"
        );
    }
    let verbatim = parse(&args(&["bazel", "build", "--offline"])).expect("verbatim");
    assert_eq!(verbatim.command, Command::Bazel);
    assert!(!verbatim.offline);
    assert_eq!(
        parse(&args(&["--offline", "bazel", "version"])),
        Err(ArgsError::UnsupportedOption {
            command: "bazel",
            option: "--offline".to_owned(),
        })
    );
    assert_eq!(
        parse(&args(&["security", "--offline=yes"])),
        Err(ArgsError::UnknownOption {
            option: "--offline=yes".to_owned(),
            suggestion: None,
        })
    );
}
