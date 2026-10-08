use super::{parse, parse_with};
use super::super::{ArgsError, Command, FileDefaults, OperationMode};
use crate::test_support::strings;
use clap::ValueEnum;

fn required_words(command: Command) -> Vec<String> {
    let required: &[&str] = match command.name() {
        "bump" => &["cargo:demo", "1.0.0"],
        "migrate" | "upgrade" => &["--from=1.0.0", "--to=2.0.0"],
        "run" | "deploy" => &["//:demo"],
        "owners" | "deps" => &["//:demo"],
        "why" => &["a.rs", "//:demo"],
        "hooks" => &["status"],
        "new" => &["rust"],
        "watch" => &["build"],
        "completion" => &["bash"],
        _ => &[],
    };
    strings(required)
}

fn with_flag(command: Command, flag: &str) -> Vec<String> {
    let mut words = vec![command.name().to_owned()];
    words.extend(required_words(command));
    words.push(flag.to_owned());
    words
}

#[test]
fn apply_matrix_matches_the_registry() {
    for command in Command::value_variants() {
        let command = *command;
        if command == Command::Bazel {
            continue;
        }
        let words = with_flag(command, "--apply");
        if command.supports_apply() {
            let got = parse(&words).unwrap_or_else(|error| {
                panic!("dx {} must take --apply: {error:?}", command.name())
            });
            assert!(got.apply, "dx {} must record --apply", command.name());
            assert_eq!(
                got.operation_mode(),
                OperationMode::Apply,
                "dx {} --apply runs in apply mode",
                command.name()
            );
        } else {
            assert_eq!(
                parse(&words),
                Err(ArgsError::UnsupportedOption {
                    command: command.name(),
                    option: "--apply".to_owned(),
                }),
                "dx {} must refuse a meaningless --apply",
                command.name()
            );
        }
    }
}

#[test]
fn bare_and_explicit_check_agree_on_check_mode() {
    for command in Command::value_variants() {
        let command = *command;
        if command == Command::Bazel {
            continue;
        }
        let mut bare_words = vec![command.name().to_owned()];
        bare_words.extend(required_words(command));
        let bare = parse(&bare_words).unwrap_or_else(|error| {
            panic!("dx {} must parse bare: {error:?}", command.name())
        });
        assert!(
            !bare.apply,
            "dx {} must not record consent it was not given",
            command.name()
        );
        assert_eq!(
            bare.operation_mode(),
            OperationMode::Check,
            "dx {} runs check by default",
            command.name()
        );
        if !command.supports_check() {
            continue;
        }
        let mut check_words = vec![command.name().to_owned()];
        check_words.extend(required_words(command));
        check_words.push("--check".to_owned());
        let checked = parse(&check_words).unwrap_or_else(|error| {
            panic!("dx {} must take --check: {error:?}", command.name())
        });
        assert!(checked.check, "dx {} must record --check", command.name());
        assert_eq!(
            checked.operation_mode(),
            bare.operation_mode(),
            "explicit --check agrees with the default for dx {}",
            command.name()
        );
    }
}

#[test]
fn dry_run_selects_plan_mode() {
    for command in Command::value_variants() {
        let command = *command;
        if command == Command::Bazel {
            continue;
        }
        let mut words = vec![command.name().to_owned()];
        words.extend(required_words(command));
        words.push("--dry-run".to_owned());
        let got = parse(&words).unwrap_or_else(|error| {
            panic!("dx {} must take --dry-run: {error:?}", command.name())
        });
        assert!(got.dry_run, "dx {} must record --dry-run", command.name());
        assert_eq!(
            got.operation_mode(),
            OperationMode::Plan,
            "dx {} --dry-run plans instead of running",
            command.name()
        );
    }
}

#[test]
fn check_and_dry_run_conflict_with_apply() {
    for command in [
        Command::Lint,
        Command::Typecheck,
        Command::Format,
        Command::Generate,
        Command::Fix,
        Command::Update,
        Command::Version,
        Command::Docs,
    ] {
        let mut words = vec![command.name().to_owned()];
        words.extend(required_words(command));
        words.push("--check".to_owned());
        words.push("--apply".to_owned());
        assert_eq!(
            parse(&words),
            Err(ArgsError::ConflictingCheckApply),
            "dx {} must refuse --check with --apply",
            command.name()
        );
    }
    for command in [
        Command::Lint,
        Command::Bump,
        Command::Clean,
        Command::Docs,
        Command::Run,
    ] {
        let mut words = vec![command.name().to_owned()];
        words.extend(required_words(command));
        words.push("--dry-run".to_owned());
        words.push("--apply".to_owned());
        assert_eq!(
            parse(&words),
            Err(ArgsError::ConflictingDryRunApply),
            "dx {} must refuse --dry-run with --apply",
            command.name()
        );
    }
}

#[test]
fn bazel_rejects_the_managed_apply_flag_but_forwards_it_after_the_separator() {
    assert_eq!(
        parse(&strings(&["bazel", "--apply"])),
        Err(ArgsError::UnsupportedOption {
            command: "bazel",
            option: "--apply".to_owned(),
        }),
        "dx bazel must reject the managed mode flag"
    );
    let forwarded = parse(&strings(&["bazel", "--", "--apply"])).expect("separator forwards");
    assert_eq!(forwarded.command, Command::Bazel);
    assert_eq!(forwarded.bazel_options, strings(&["--apply"]));
    assert!(
        !forwarded.apply,
        "a forwarded --apply must not read as dx consent"
    );
    assert!(
        !Command::Bazel.supports_apply(),
        "the unmanaged passthrough takes no managed mode"
    );
}

#[test]
fn no_default_source_selects_apply() {
    assert!(
        !dx_adopt::defaults::ENV_DEFAULTS
            .iter()
            .any(|(_, flag, _)| *flag == "--apply"),
        "no environment default may select apply"
    );
    assert!(
        dx_adopt::defaults::ENV_DEFAULTS
            .iter()
            .all(|(env, _, _)| !env.contains("APPLY")),
        "no environment default may name apply"
    );
    assert!(
        dx_adopt::defaults::parse_file_text("[dx]\napply = true\n").is_err(),
        "a defaults file must not carry an apply key"
    );
    let hostile = |name: &str| {
        if name == dx_adopt::defaults::DX_DRY_RUN_ENV {
            Some("1".to_owned())
        } else {
            None
        }
    };
    assert_eq!(
        parse_with(
            &strings(&["lint", "--apply"]),
            &hostile,
            &FileDefaults::default(),
        ),
        Err(ArgsError::ConflictingDryRunApply),
        "an inherited dry run still conflicts with --apply"
    );
    let apply_env = |name: &str| {
        if name == "DX_APPLY" {
            Some("1".to_owned())
        } else {
            None
        }
    };
    let got = parse_with(&strings(&["lint"]), &apply_env, &FileDefaults::default())
        .expect("an unknown apply variable changes nothing");
    assert!(
        !got.apply,
        "no environment value may silently select apply"
    );
    assert_eq!(got.operation_mode(), OperationMode::Check);
    let file = dx_adopt::defaults::FileDefaults {
        dry_run: Some(true),
        ..Default::default()
    };
    assert_eq!(
        parse_with(&strings(&["lint", "--apply"]), &|_| None, &file),
        Err(ArgsError::ConflictingDryRunApply),
        "a file-level dry run still conflicts with --apply"
    );
}
