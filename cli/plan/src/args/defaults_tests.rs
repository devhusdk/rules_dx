use super::super::{Command, OperationMode};
use super::parse_with;
use dx_adopt::defaults::ENV_DEFAULTS;
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
fn env_supplies_output() {
    let env = env_of(&[("DX_OUTPUT", "json")]);
    let got = parse_with(&strings(&["lint"]), &env).expect("env parse");
    assert_eq!(got.output, OutputMode::Json);
}

#[test]
fn unsupported_defaults_fall_back_per_command() {
    let env = env_of(&[("DX_OUTPUT", "json")]);
    for words in [
        &["build"][..],
        &["clean"][..],
        &["init"][..],
        &["completion", "bash"][..],
    ] {
        let got = parse_with(&strings(words), &env)
            .unwrap_or_else(|error| panic!("{words:?}: {error:?}"));
        if got.command.supports_json() {
            assert_eq!(got.output, OutputMode::Json, "{words:?}");
        } else {
            assert_eq!(got.output, OutputMode::Text { quiet: false }, "{words:?}");
        }
    }
    let got = parse_with(&strings(&["lint"]), &env).expect("lint keeps defaults");
    assert_eq!(got.output, OutputMode::Json);
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
fn explicit_unsupported_flags_and_bad_output_default_stay_errors() {
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
        parse_with(&strings(&["init"]), &env_of(&[("DX_OUTPUT", "yaml")])),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
}

#[test]
fn precedence_is_flag_over_env() {
    let env = env_of(&[("DX_OUTPUT", "json")]);
    let got = parse_with(&strings(&["lint", "--output=text"]), &env).expect("flag wins");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got = parse_with(&strings(&["lint"]), &env).expect("env wins");
    assert_eq!(got.output, OutputMode::Json);
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
    let env = env_of(&[("DX_OUTPUT", "")]);
    let got = parse_with(&strings(&["lint"]), &env).expect("empty env absent");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
}

#[test]
fn color_flag_is_validated() {
    let got = parse_with(&strings(&["lint", "--color=never"]), &env_of(&[])).expect("flag");
    assert_eq!(got.color, ColorMode::Never);
    assert!(parse_with(&strings(&["lint", "--color=bright"]), &env_of(&[])).is_err());
}

#[test]
fn quiet_flows_into_text_mode() {
    let got = parse_with(&strings(&["lint", "--quiet"]), &env_of(&[])).expect("quiet");
    assert!(got.quiet);
    assert_eq!(got.output, OutputMode::Text { quiet: true });
}

#[test]
fn quiet_and_verbose_take_no_value() {
    for (flag, verbose) in [
        ("--quiet", false),
        ("-q", false),
        ("--verbose", true),
        ("-v", true),
    ] {
        let got = parse_with(&strings(&["lint", flag]), &env_of(&[])).expect(flag);
        assert_eq!(got.verbose, verbose, "{flag}");
        assert_eq!(got.quiet, !verbose, "{flag}");
    }
    for flag in ["--quiet=false", "--verbose=false", "-v=false"] {
        assert!(
            parse_with(&strings(&["lint", flag]), &env_of(&[])).is_err(),
            "{flag}"
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
fn every_agent_has_a_marker_or_a_reason() {
    use super::super::grammar::Agent;
    for agent in Agent::ALL {
        let marker = match agent {
            Agent::Claude => Some("CLAUDECODE"),
            Agent::Codex => Some("CODEX_THREAD_ID"),
            Agent::Gemini => Some("GEMINI_CLI"),
            // GitHub documents no variable that marks a Copilot CLI shell.
            Agent::Copilot => None,
        };
        if let Some(marker) = marker {
            assert!(dx_adopt::defaults::AGENT_ENVS.contains(&marker), "{agent:?}");
        }
    }
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
    let env = env_of(&[("DX_OUTPUT", "json")]);
    let got = parse_with_ci(&strings(&["lint"]), &env, true).expect("ci parses");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got =
        parse_with_ci(&strings(&["lint", "--output=json"]), &env, true).expect("flags win in ci");
    assert_eq!(got.output, OutputMode::Json);
    let got = parse_with(&strings(&["lint"]), &env).expect("non-ci reads env");
    assert_eq!(got.output, OutputMode::Json);
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

#[test]
fn build_slots_wait_reads_flag_then_env_and_rejects_bad_values() {
    let wait = |words: &[&str], env: &[(&str, &str)]| {
        parse_with(&strings(words), &env_of(env)).map(|invocation| invocation.build_slots_wait)
    };
    let secs = |n| Some(std::time::Duration::from_secs(n));
    assert_eq!(wait(&["build"], &[]).expect("ok"), None);
    assert_eq!(
        wait(&["build"], &[("DX_BUILD_SLOTS_WAIT", "2m")]).expect("ok"),
        secs(120)
    );
    assert_eq!(
        wait(
            &["build", "--build-slots-wait", "0"],
            &[("DX_BUILD_SLOTS_WAIT", "2m")]
        )
        .expect("ok"),
        secs(0)
    );
    assert_eq!(
        wait(&["build", "--build-slots-wait", "soon"], &[]),
        Err(crate::args::ArgsError::BadBuildSlotsWait {
            origin: "--build-slots-wait",
            value: "soon".to_owned()
        })
    );
}
