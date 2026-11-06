use super::super::{Command, OperationMode};
use super::parse_with;
use dx_adopt::defaults::{BOOL_SPELLINGS, DX_QUIET_ENV, DX_VERBOSE_ENV, ENV_DEFAULTS, parse_bool};
use dx_output::{ColorMode, OutputMode, Threshold};
use dx_testing::cli::strings;

fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// An environment getter that owns its pairs, so it outlives a temporary slice.
fn owned_env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    move |name| {
        owned
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
}

#[test]
fn flag_only_parses_with_builtin_defaults() {
    let got = parse_with(&strings(&["lint"]), &env_of(&[])).expect("parse");
    assert_eq!(got.command, Command::Lint);
    assert_eq!(got.workspace, None);
    assert!(!got.verbose);
    assert_eq!(got.color, ColorMode::Auto);
    assert!(!got.quiet);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert_eq!(got.fail_on, Threshold::Warning);
}

#[test]
fn env_supplies_workspace_output_and_bools() {
    let env = env_of(&[
        ("DX_WORKSPACE", "/repo"),
        ("DX_OUTPUT", "json"),
        ("DX_VERBOSE", "true"),
        ("DX_QUIET", "true"),
        ("DX_FAIL_ON", "error"),
    ]);
    let got = parse_with(&strings(&["lint"]), &env).expect("env parse");
    assert_eq!(got.workspace, Some("/repo".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    assert!(got.quiet);
    assert_eq!(got.fail_on, Threshold::Error);
    assert_eq!(got.output, OutputMode::Json);
}

#[test]
fn unsupported_defaults_fall_back_per_command() {
    let env = env_of(&[("DX_OUTPUT", "json"), ("DX_FAIL_ON", "error")]);
    for words in [
        &["build"][..],
        &["clean"][..],
        &["init"][..],
        &["completion", "bash"][..],
    ] {
        let got = parse_with(&strings(words), &env)
            .unwrap_or_else(|error| panic!("{words:?}: {error:?}"));
        assert_eq!(got.fail_on, Threshold::Warning, "{words:?}");
        if got.command.supports_json() {
            assert_eq!(got.output, OutputMode::Json, "{words:?}");
        } else {
            assert_eq!(got.output, OutputMode::Text { quiet: false }, "{words:?}");
        }
    }
    let got = parse_with(&strings(&["lint"]), &env).expect("lint keeps defaults");
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(got.fail_on, Threshold::Error);
}

#[test]
fn path_only_is_a_worktree_add_flag() {
    let got = parse_with(
        &strings(&["worktree", "add", "topic", "--path-only"]),
        &env_of(&[]),
    )
    .expect("add path-only");
    assert!(got.path_only);
    let got = parse_with(&strings(&["worktree", "add", "topic"]), &env_of(&[])).expect("add");
    assert!(!got.path_only);
    assert!(parse_with(&strings(&["worktree", "list", "--path-only"]), &env_of(&[])).is_err());
}

#[test]
fn explicit_unsupported_flags_and_bad_defaults_stay_errors() {
    use super::super::ArgsError;
    assert_eq!(
        parse_with(&strings(&["build", "--fail-on=error"]), &env_of(&[])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--fail-on".to_owned(),
        })
    );
    assert_eq!(
        parse_with(&strings(&["build", "--diff"]), &env_of(&[])),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--diff".to_owned(),
        })
    );
    assert_eq!(
        parse_with(&strings(&["build"]), &env_of(&[("DX_FAIL_ON", "never")])),
        Err(ArgsError::BadFailOn {
            value: "never".to_owned(),
        })
    );
    assert_eq!(
        parse_with(&strings(&["init"]), &env_of(&[("DX_OUTPUT", "yaml")])),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
}

#[test]
fn precedence_is_flag_over_env() {
    let env = env_of(&[("DX_WORKSPACE", "/env"), ("DX_OUTPUT", "json")]);
    let got = parse_with(
        &strings(&["lint", "--workspace", "/flag", "--output=text"]),
        &env,
    )
    .expect("flag wins");
    assert_eq!(got.workspace, Some("/flag".to_owned()));
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got = parse_with(&strings(&["lint"]), &env).expect("env wins");
    assert_eq!(got.workspace, Some("/env".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    let env_false = env_of(&[("DX_VERBOSE", "false")]);
    let got = parse_with(&strings(&["lint", "--verbose"]), &env_false).expect("flag bool wins");
    assert!(got.verbose);
}

#[test]
fn invalid_env_values_fail_closed() {
    use super::super::ArgsError;
    let env = env_of(&[("DX_OUTPUT", "yaml")]);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
    let env = env_of(&[("DX_FAIL_ON", "never")]);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env),
        Err(ArgsError::BadFailOn {
            value: "never".to_owned(),
        })
    );
    let env = env_of(&[("DX_COLOR", "bright")]);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env),
        Err(ArgsError::BadColor {
            value: "bright".to_owned(),
        })
    );
    let env = env_of(&[("DX_WORKSPACE", ""), ("DX_OUTPUT", "")]);
    let got = parse_with(&strings(&["lint"]), &env).expect("empty env absent");
    assert_eq!(got.workspace, None);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
}

#[test]
fn color_flag_env_precedence() {
    let env = env_of(&[("DX_COLOR", "always")]);
    let got = parse_with(&strings(&["lint", "--color=never"]), &env).expect("flag wins");
    assert_eq!(got.color, ColorMode::Never);
    let got = parse_with(&strings(&["lint"]), &env).expect("env wins");
    assert_eq!(got.color, ColorMode::Always);
    assert!(parse_with(&strings(&["lint", "--color=bright"]), &env_of(&[])).is_err());
}

#[test]
fn quiet_flows_into_text_mode() {
    let env = env_of(&[(DX_QUIET_ENV, "true")]);
    let got = parse_with(&strings(&["lint"]), &env).expect("quiet env");
    assert!(got.quiet);
    assert_eq!(got.output, OutputMode::Text { quiet: true });
}

#[test]
fn a_flag_turns_an_inherited_boolean_off() {
    let inherited = env_of(&[(DX_QUIET_ENV, "true"), (DX_VERBOSE_ENV, "true")]);
    let off = |flag: &str| {
        parse_with(&strings(&["lint", flag]), &inherited)
            .unwrap_or_else(|error| panic!("{flag}: {error:?}"))
    };
    assert!(
        !off("--quiet=false").quiet,
        "flag false beats the environment"
    );
    assert!(
        !off("--verbose=false").verbose,
        "flag false beats the environment"
    );
    assert!(!off("-v=false").verbose, "the short form takes a value too");
    let on = |flag: &str| {
        parse_with(&strings(&["lint", flag]), &env_of(&[]))
            .unwrap_or_else(|error| panic!("{flag}: {error:?}"))
    };
    assert!(on("--quiet").quiet, "the bare form stays true");
    assert!(on("--verbose").verbose, "the bare form stays true");
    assert!(on("-v").verbose, "the bare short form stays true");
    let got = parse_with(
        &strings(&["lint", "--quiet", "--quiet=false"]),
        &env_of(&[]),
    )
    .expect("last one wins");
    assert!(!got.quiet, "the last occurrence wins");
    for (env_name, index) in [(DX_QUIET_ENV, 0), (DX_VERBOSE_ENV, 1)] {
        for value in ["false", "FALSE"] {
            let env = owned_env(&[(env_name, value)]);
            let got = parse_with(&strings(&["lint"]), &env)
                .unwrap_or_else(|error| panic!("{env_name}={value}: {error:?}"));
            assert!(![got.quiet, got.verbose][index], "{env_name}={value}");
        }
        for value in ["true", "TRUE"] {
            let env = owned_env(&[(env_name, value)]);
            let got = parse_with(&strings(&["lint"]), &env)
                .unwrap_or_else(|error| panic!("{env_name}={value}: {error:?}"));
            assert!(
                [got.quiet, got.verbose][index],
                "{env_name}={value} turns the default on"
            );
        }
    }
    for word in ["true", "false", "TRUE", "FALSE", " true "] {
        let env = owned_env(&[(DX_QUIET_ENV, word)]);
        let got = parse_with(&strings(&["lint"]), &env)
            .unwrap_or_else(|error| panic!("DX_QUIET={word}: {error:?}"));
        assert_eq!(got.quiet, parse_bool(word) == Some(true), "{word}");
    }
}

#[test]
fn a_boolean_value_outside_the_spellings_is_a_usage_error() {
    use super::super::ArgsError;
    for (env_name, value) in [
        (DX_QUIET_ENV, "tru"),
        (DX_QUIET_ENV, "ture"),
        (DX_VERBOSE_ENV, "2"),
        (DX_VERBOSE_ENV, "maybe"),
        (DX_VERBOSE_ENV, "1"),
        (DX_QUIET_ENV, "yes"),
        (DX_VERBOSE_ENV, "y"),
        (DX_VERBOSE_ENV, "on"),
        (DX_QUIET_ENV, "0"),
        (DX_VERBOSE_ENV, "no"),
        (DX_VERBOSE_ENV, "n"),
        (DX_QUIET_ENV, "off"),
    ] {
        let env = owned_env(&[(env_name, value)]);
        let error = parse_with(&strings(&["lint"]), &env).expect_err(value);
        assert_eq!(
            error,
            ArgsError::BadDefault {
                detail: format!(
                    "invalid invocation default {env_name}={value:?}: want one of {BOOL_SPELLINGS}"
                )
            },
            "{env_name}={value}"
        );
        assert!(
            error.to_string().contains(&format!("{env_name}={value:?}")),
            "{error}"
        );
    }
    for value in ["", "true", "false", "TRUE", " true "] {
        let env = owned_env(&[(DX_QUIET_ENV, value)]);
        assert!(
            parse_with(&strings(&["lint"]), &env).is_ok(),
            "DX_QUIET={value:?} must parse"
        );
    }
}

#[test]
fn no_default_source_selects_apply() {
    for (env, _, _) in ENV_DEFAULTS {
        assert_ne!(env, "DX_APPLY", "no environment default may select apply");
    }
    let got = parse_with(&strings(&["lint"]), &env_of(&[("DX_APPLY", "1")]))
        .expect("an unknown variable is unread");
    assert!(!got.apply);
    assert_eq!(got.operation(), OperationMode::Check);
}

#[test]
fn agent_markers_default_json_and_explicit_choices_win() {
    use super::parse_with_ci;
    for marker in dx_adopt::defaults::AGENT_ENVS {
        let pairs = [(marker, "1")];
        let env = env_of(&pairs);
        let got = parse_with(&strings(&["test"]), &env).expect("agent");
        assert_eq!(got.output, OutputMode::Json, "{marker}");
        assert!(got.agent, "{marker}");
    }
    let env = env_of(&[("CLAUDECODE", "1")]);
    let got = parse_with(&strings(&["test", "--output=text"]), &env).expect("flag wins");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let env = env_of(&[("CLAUDECODE", "1"), ("DX_OUTPUT", "text")]);
    let got = parse_with(&strings(&["test"]), &env).expect("env wins");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let env = env_of(&[("CLAUDECODE", "")]);
    let got = parse_with(&strings(&["test"]), &env).expect("empty");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(!got.agent);
    let env = env_of(&[("CLAUDECODE", "1")]);
    let got = parse_with_ci(&strings(&["test"]), &env, true).expect("ci ignores agents");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(!got.agent);
}

#[test]
fn init_apply_ignores_the_agent_json_default() {
    let env = env_of(&[("CLAUDECODE", "1")]);
    let got = parse_with(&strings(&["init", "--apply"]), &env).expect("apply");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got = parse_with(&strings(&["init"]), &env).expect("plan");
    assert_eq!(got.output, OutputMode::Json);
    let got = parse_with(&strings(&["init", "--apply", "--output=json"]), &env).expect("parses");
    assert_eq!(got.output, OutputMode::Json);
}

#[test]
fn ci_ignores_preference_env_but_keeps_flags() {
    use super::parse_with_ci;
    let env = env_of(&[("DX_OUTPUT", "json"), ("DX_VERBOSE", "true")]);
    let got = parse_with_ci(&strings(&["lint"]), &env, true).expect("ci parses");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(!got.verbose);
    let got = parse_with_ci(
        &strings(&["lint", "--output=json", "--verbose"]),
        &env,
        true,
    )
    .expect("flags win in ci");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    let got = parse_with(&strings(&["lint"]), &env).expect("non-ci reads env");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
}

fn slots(n: usize) -> dx_adopt::defaults::BuildSlots {
    dx_adopt::defaults::BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"))
}

fn resolved_slots(words: &[&str], env: &[(&str, &str)]) -> Option<dx_adopt::defaults::BuildSlots> {
    parse_with(&strings(words), &env_of(env))
        .expect("parse")
        .build_slots
}

#[test]
fn build_slots_default_is_unset() {
    assert_eq!(resolved_slots(&["build"], &[]), None);
}

#[test]
fn build_slots_flag_beats_env() {
    let env = [("DX_BUILD_SLOTS", "3")];
    assert_eq!(
        resolved_slots(&["build", "--build-slots=2"], &env),
        Some(slots(2))
    );
    assert_eq!(
        resolved_slots(&["build", "--build-slots", "off"], &env),
        Some(dx_adopt::defaults::BuildSlots::Off)
    );
}

#[test]
fn build_slots_empty_env_is_unset() {
    assert_eq!(resolved_slots(&["build"], &[("DX_BUILD_SLOTS", "")]), None);
}

#[test]
fn ci_ignores_build_slots_env_but_keeps_flag() {
    use super::parse_with_ci;
    let env = env_of(&[("DX_BUILD_SLOTS", "off")]);
    let got = parse_with_ci(&strings(&["build"]), &env, true).expect("ci");
    assert_eq!(got.build_slots, None);
    let got = parse_with_ci(&strings(&["build", "--build-slots=2"]), &env, true).expect("ci flag");
    assert_eq!(got.build_slots, Some(slots(2)));
}

#[test]
fn bad_build_slots_are_usage_errors() {
    use super::super::ArgsError;
    for (words, env, source, value) in [
        (
            vec!["build", "--build-slots=-1"],
            vec![],
            "--build-slots",
            "-1",
        ),
        (
            vec!["build", "--build-slots", "-1"],
            vec![],
            "--build-slots",
            "-1",
        ),
        (
            vec!["build", "--build-slots", "0"],
            vec![],
            "--build-slots",
            "0",
        ),
        (
            vec!["build"],
            vec![("DX_BUILD_SLOTS", "many")],
            "DX_BUILD_SLOTS",
            "many",
        ),
    ] {
        let error = parse_with(&strings(&words), &env_of(&env)).expect_err("bad");
        assert_eq!(
            error,
            ArgsError::BadBuildSlots {
                origin: source,
                value: value.to_owned()
            },
            "{words:?} {env:?}"
        );
        assert!(error.to_string().contains("auto|<n>|off"), "{error}");
    }
}

#[test]
fn build_slots_flag_is_accepted_on_every_command() {
    assert_eq!(
        resolved_slots(&["build", "--build-slots=2"], &[]),
        Some(slots(2))
    );
    assert_eq!(
        resolved_slots(&["status", "--build-slots=off"], &[]),
        Some(dx_adopt::defaults::BuildSlots::Off)
    );
    assert_eq!(
        resolved_slots(&["lint", "--build-slots", "3", "//..."], &[]),
        Some(slots(3))
    );
}
