use super::super::{Command, FileDefaults, OperationMode};
use super::parse_with;
use dx_adopt::defaults::{
    config_key, parse_bool, parse_file_text, BOOL_SPELLINGS, DX_QUIET_ENV, DX_VERBOSE_ENV,
    ENV_DEFAULTS,
};
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
fn owned_env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
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

fn file_with(
    workspace: Option<&str>,
    output: Option<&str>,
    verbose: Option<bool>,
    quiet: Option<bool>,
    fail_on: Option<&str>,
) -> FileDefaults {
    FileDefaults {
        workspace: workspace.map(ToString::to_string),
        output: output.map(ToString::to_string),
        verbose,
        color: None,
        quiet,
        fail_on: fail_on.map(ToString::to_string),
        build_slots: None,
        baseline: None,
    }
}

#[test]
fn flag_only_parses_with_builtin_defaults() {
    let empty = FileDefaults::default();
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &empty).expect("parse");
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
    let empty = FileDefaults::default();
    let env = env_of(&[
        ("DX_WORKSPACE", "/repo"),
        ("DX_OUTPUT", "json"),
        ("DX_VERBOSE", "true"),
        ("DX_QUIET", "true"),
        ("DX_FAIL_ON", "error"),
    ]);
    let got = parse_with(&strings(&["lint"]), &env, &empty).expect("env parse");
    assert_eq!(got.workspace, Some("/repo".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    assert!(got.quiet);
    assert_eq!(got.fail_on, Threshold::Error);
    assert_eq!(got.output, OutputMode::Json);
}

#[test]
fn file_supplies_defaults_when_flag_and_env_absent() {
    let file = file_with(
        Some("/file-ws"),
        Some("json"),
        Some(true),
        Some(true),
        Some("error"),
    );
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &file).expect("file parse");
    assert_eq!(got.workspace, Some("/file-ws".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    assert!(got.quiet);
    assert_eq!(got.fail_on, Threshold::Error);
}

#[test]
fn unsupported_defaults_fall_back_per_command() {
    let file = file_with(None, Some("json"), None, None, Some("error"));
    let env = env_of(&[("DX_OUTPUT", "json"), ("DX_FAIL_ON", "error")]);
    let empty = FileDefaults::default();
    for words in [
        &["build"][..],
        &["clean"][..],
        &["init"][..],
        &["completion", "bash"][..],
    ] {
        for (label, got) in [
            ("file", parse_with(&strings(words), &env_of(&[]), &file)),
            ("env", parse_with(&strings(words), &env, &empty)),
        ] {
            let got = got.unwrap_or_else(|error| panic!("{words:?} {label}: {error:?}"));
            assert_eq!(got.fail_on, Threshold::Warning, "{words:?} {label}");
            if got.command.supports_json() {
                assert_eq!(got.output, OutputMode::Json, "{words:?} {label}");
            } else {
                assert_eq!(
                    got.output,
                    OutputMode::Text { quiet: false },
                    "{words:?} {label}"
                );
            }
        }
    }
    let diff = file_with(None, Some("diff"), None, None, None);
    let got = parse_with(&strings(&["build"]), &env_of(&[]), &diff).expect("file diff on build");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got = parse_with(
        &strings(&["build"]),
        &env_of(&[("DX_OUTPUT", "diff")]),
        &empty,
    )
    .expect("env diff on build");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got = parse_with(&strings(&["test"]), &env_of(&[("CLAUDECODE", "1")]), &diff)
        .expect("agent diff on test");
    assert_eq!(got.output, OutputMode::Json);
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &file).expect("lint keeps defaults");
    assert_eq!(got.output, OutputMode::Json);
    assert_eq!(got.fail_on, Threshold::Error);
    let path = file_with(None, Some("path"), None, None, None);
    let path_env = env_of(&[("DX_OUTPUT", "path")]);
    for words in [
        &["build"][..],
        &["clean"][..],
        &["init"][..],
        &["lint"][..],
        &["completion", "bash"][..],
    ] {
        for (label, got) in [
            ("file", parse_with(&strings(words), &env_of(&[]), &path)),
            ("env", parse_with(&strings(words), &path_env, &empty)),
        ] {
            let got = got.unwrap_or_else(|error| panic!("{words:?} {label}: {error:?}"));
            assert_eq!(
                got.output,
                OutputMode::Text { quiet: false },
                "{words:?} {label}"
            );
            assert!(!got.path_only, "{words:?} {label}");
        }
        let got = parse_with(&strings(words), &env_of(&[("CLAUDECODE", "1")]), &path)
            .unwrap_or_else(|error| panic!("{words:?} agent: {error:?}"));
        let want = if got.command.supports_json() {
            OutputMode::Json
        } else {
            OutputMode::Text { quiet: false }
        };
        assert_eq!(got.output, want, "{words:?} agent");
    }
}

#[test]
fn path_default_applies_to_worktree_add_only() {
    let path = file_with(None, Some("path"), None, None, None);
    let path_env = env_of(&[("DX_OUTPUT", "path")]);
    let empty = FileDefaults::default();
    for (label, env, file) in [
        (
            "file",
            &env_of(&[]) as &dyn Fn(&str) -> Option<String>,
            &path,
        ),
        ("env", &path_env, &empty),
    ] {
        let got = parse_with(&strings(&["worktree", "add", "topic"]), env, file)
            .unwrap_or_else(|error| panic!("add {label}: {error:?}"));
        assert!(got.path_only, "add {label}");
        assert_eq!(got.output, OutputMode::Text { quiet: false }, "add {label}");
        for words in [
            &["worktree"][..],
            &["worktree", "list"][..],
            &["worktree", "path", "topic"][..],
        ] {
            let got = parse_with(&strings(words), env, file)
                .unwrap_or_else(|error| panic!("{words:?} {label}: {error:?}"));
            assert!(!got.path_only, "{words:?} {label}");
            assert_eq!(
                got.output,
                OutputMode::Text { quiet: false },
                "{words:?} {label}"
            );
        }
    }
    let agent_path = env_of(&[("CLAUDECODE", "1"), ("DX_OUTPUT", "path")]);
    let got = parse_with(&strings(&["worktree", "list"]), &agent_path, &empty)
        .expect("agent worktree list");
    assert_eq!(got.output, OutputMode::Json);
    assert!(!got.path_only);
    let got = parse_with(&strings(&["worktree", "add", "topic"]), &agent_path, &empty)
        .expect("agent worktree add");
    assert!(got.path_only);
    let got = parse_with(
        &strings(&["worktree", "list", "--output=path"]),
        &env_of(&[]),
        &empty,
    )
    .expect("explicit path flag");
    assert!(got.path_only);
}

#[test]
fn explicit_unsupported_flags_and_bad_defaults_stay_errors() {
    use super::super::ArgsError;
    let empty = FileDefaults::default();
    assert_eq!(
        parse_with(
            &strings(&["build", "--fail-on=error"]),
            &env_of(&[]),
            &empty
        ),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--fail-on".to_owned(),
        })
    );
    assert_eq!(
        parse_with(&strings(&["init", "--output=json"]), &env_of(&[]), &empty),
        Err(ArgsError::UnsupportedOption {
            command: "init",
            option: "--output=json".to_owned(),
        })
    );
    assert_eq!(
        parse_with(&strings(&["build", "--output=diff"]), &env_of(&[]), &empty),
        Err(ArgsError::UnsupportedOption {
            command: "build",
            option: "--output=diff".to_owned(),
        })
    );
    let never = file_with(None, None, None, None, Some("never"));
    assert_eq!(
        parse_with(&strings(&["build"]), &env_of(&[]), &never),
        Err(ArgsError::BadFailOn {
            value: "never".to_owned(),
        })
    );
    assert_eq!(
        parse_with(
            &strings(&["init"]),
            &env_of(&[("DX_OUTPUT", "yaml")]),
            &empty
        ),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
}

#[test]
fn precedence_is_flag_over_env_over_file() {
    let file = file_with(
        Some("/file"),
        Some("diff"),
        Some(true),
        Some(false),
        Some("info"),
    );
    let env = env_of(&[("DX_WORKSPACE", "/env"), ("DX_OUTPUT", "json")]);
    let got = parse_with(
        &strings(&["lint", "--workspace", "/flag", "--output=text"]),
        &env,
        &file,
    )
    .expect("flag wins");
    assert_eq!(got.workspace, Some("/flag".to_owned()));
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let got = parse_with(&strings(&["lint"]), &env, &file).expect("env wins");
    assert_eq!(got.workspace, Some("/env".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    let file_bools = file_with(None, None, Some(true), None, None);
    let got = parse_with(&strings(&["lint", "--verbose"]), &env_of(&[]), &file_bools)
        .expect("flag bool wins");
    assert!(got.verbose);
    let env_true = env_of(&[("DX_VERBOSE", "true")]);
    let got = parse_with(
        &strings(&["lint"]),
        &env_true,
        &file_with(None, None, Some(false), None, None),
    )
    .expect("env bool wins");
    assert!(got.verbose);
    let env_false = env_of(&[("DX_VERBOSE", "false")]);
    let got = parse_with(
        &strings(&["lint"]),
        &env_false,
        &file_with(None, None, Some(true), None, None),
    )
    .expect("env falsy disables file");
    assert!(!got.verbose);
}

#[test]
fn invalid_env_and_file_values_fail_closed() {
    use super::super::ArgsError;
    let env = env_of(&[("DX_OUTPUT", "yaml")]);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env, &FileDefaults::default()),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
    let env = env_of(&[("DX_FAIL_ON", "never")]);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env, &FileDefaults::default()),
        Err(ArgsError::BadFailOn {
            value: "never".to_owned(),
        })
    );
    let env = env_of(&[("DX_COLOR", "bright")]);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env, &FileDefaults::default()),
        Err(ArgsError::BadColor {
            value: "bright".to_owned(),
        })
    );
    let file = file_with(None, Some("yaml"), None, None, None);
    assert_eq!(
        parse_with(&strings(&["lint"]), &env_of(&[]), &file),
        Err(ArgsError::BadOutput {
            value: "yaml".to_owned(),
        })
    );
    let env = env_of(&[("DX_WORKSPACE", ""), ("DX_OUTPUT", "")]);
    let got =
        parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).expect("empty env absent");
    assert_eq!(got.workspace, None);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
}

#[test]
fn color_flag_env_file_precedence() {
    let mut file = FileDefaults::default();
    file.color = Some("never".to_owned());
    let env = env_of(&[("DX_COLOR", "always")]);
    let got = parse_with(&strings(&["lint", "--color=never"]), &env, &file).expect("flag wins");
    assert_eq!(got.color, ColorMode::Never);
    let got = parse_with(&strings(&["lint"]), &env, &file).expect("env wins");
    assert_eq!(got.color, ColorMode::Always);
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &file).expect("file wins");
    assert_eq!(got.color, ColorMode::Never);
    assert!(parse_with(
        &strings(&["lint", "--color=bright"]),
        &env_of(&[]),
        &FileDefaults::default()
    )
    .is_err());
}

#[test]
fn quiet_flows_into_text_mode() {
    let env = env_of(&[(DX_QUIET_ENV, "true")]);
    let got = parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).expect("quiet env");
    assert!(got.quiet);
    assert_eq!(got.output, OutputMode::Text { quiet: true });
}

#[test]
fn a_flag_turns_an_inherited_boolean_off() {
    let file = file_with(None, None, Some(true), Some(true), None);
    let off = |flag: &str| {
        parse_with(&strings(&["lint", flag]), &env_of(&[]), &file)
            .unwrap_or_else(|error| panic!("{flag}: {error:?}"))
    };
    assert!(!off("--quiet=false").quiet, "flag false beats the file");
    assert!(!off("--verbose=false").verbose, "flag false beats the file");
    assert!(!off("-v=false").verbose, "the short form takes a value too");
    let on = |flag: &str| {
        parse_with(&strings(&["lint", flag]), &env_of(&[]), &file)
            .unwrap_or_else(|error| panic!("{flag}: {error:?}"))
    };
    assert!(on("--quiet").quiet, "the bare form stays true");
    assert!(on("--verbose").verbose, "the bare form stays true");
    assert!(on("-v").verbose, "the bare short form stays true");
    let env = env_of(&[(DX_VERBOSE_ENV, "true")]);
    let got = parse_with(&strings(&["lint", "--verbose=false"]), &env, &file)
        .expect("flag false beats the environment");
    assert!(!got.verbose, "the flag beats the environment");
    let got = parse_with(
        &strings(&["lint", "--quiet", "--quiet=false"]),
        &env_of(&[]),
        &file,
    )
    .expect("last one wins");
    assert!(!got.quiet, "the last occurrence wins");
    for (env_name, index) in [(DX_QUIET_ENV, 0), (DX_VERBOSE_ENV, 1)] {
        for value in ["false", "FALSE"] {
            let env = owned_env(&[(env_name, value)]);
            let got = parse_with(&strings(&["lint"]), &env, &file)
                .unwrap_or_else(|error| panic!("{env_name}={value}: {error:?}"));
            let flags = [got.quiet, got.verbose];
            assert!(
                !flags[index],
                "{env_name}={value} turns the file default off: {flags:?}"
            );
            let others: Vec<bool> = flags
                .iter()
                .enumerate()
                .filter(|(slot, _)| *slot != index)
                .map(|(_, flag)| *flag)
                .collect();
            assert!(
                others.iter().all(|flag| *flag),
                "{env_name}={value} leaves the other defaults alone: {flags:?}"
            );
        }
        for value in ["true", "TRUE"] {
            let env = owned_env(&[(env_name, value)]);
            let got = parse_with(&strings(&["lint"]), &env, &FileDefaults::default())
                .unwrap_or_else(|error| panic!("{env_name}={value}: {error:?}"));
            assert_eq!(
                [got.quiet, got.verbose][index],
                true,
                "{env_name}={value} turns the default on"
            );
        }
    }
    for word in ["true", "false", "TRUE", "FALSE", " true "] {
        let env = owned_env(&[(DX_QUIET_ENV, word)]);
        let got = parse_with(&strings(&["lint"]), &env, &FileDefaults::default())
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
        let error =
            parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).expect_err(value);
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
            parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).is_ok(),
            "DX_QUIET={value:?} must parse"
        );
    }
}

#[test]
fn boolean_precedence_runs_cli_then_env_then_dx_table() {
    let table_only = parse_file_text("[dx]\nquiet = true\n").expect("dx table");
    assert_eq!(table_only.quiet, Some(true));
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &FileDefaults::default())
        .expect("the built-in default is off");
    assert!(!got.quiet && !got.verbose);
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &table_only).expect("dx table");
    assert!(got.quiet, "[dx] beats the built-in default");
    let env = env_of(&[(DX_QUIET_ENV, "false")]);
    let got = parse_with(&strings(&["lint"]), &env, &table_only).expect("env");
    assert!(!got.quiet, "the environment beats the file");
    let got = parse_with(&strings(&["lint", "--quiet=false"]), &env, &table_only).expect("flag");
    assert!(!got.quiet, "the flag beats the environment");
}

#[test]
fn no_default_source_selects_apply() {
    for (env, _, _) in ENV_DEFAULTS {
        assert_ne!(env, "DX_APPLY", "no environment default may select apply");
    }
    assert_eq!(config_key("DX_APPLY"), None);
    let got = parse_with(
        &strings(&["lint"]),
        &env_of(&[("DX_APPLY", "1")]),
        &FileDefaults::default(),
    )
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
        let got = parse_with(&strings(&["test"]), &env, &FileDefaults::default()).expect("agent");
        assert_eq!(got.output, OutputMode::Json, "{marker}");
        assert!(got.agent, "{marker}");
    }
    let env = env_of(&[("CLAUDECODE", "1")]);
    let got = parse_with(
        &strings(&["test", "--output=text"]),
        &env,
        &FileDefaults::default(),
    )
    .expect("flag wins");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let env = env_of(&[("CLAUDECODE", "1"), ("DX_OUTPUT", "text")]);
    let got = parse_with(&strings(&["test"]), &env, &FileDefaults::default()).expect("env wins");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    let env = env_of(&[("CLAUDECODE", "")]);
    let got = parse_with(&strings(&["test"]), &env, &FileDefaults::default()).expect("empty");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(!got.agent);
    let env = env_of(&[("CLAUDECODE", "1")]);
    let got = parse_with_ci(&strings(&["test"]), &env, &FileDefaults::default(), true)
        .expect("ci ignores agents");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(!got.agent);
}

#[test]
fn ci_ignores_preference_env_but_keeps_flags() {
    use super::parse_with_ci;
    let env = env_of(&[("DX_OUTPUT", "json"), ("DX_VERBOSE", "true")]);
    let got = parse_with_ci(&strings(&["lint"]), &env, &FileDefaults::default(), true)
        .expect("ci parses");
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert!(!got.verbose);
    let got = parse_with_ci(
        &strings(&["lint", "--output=json", "--verbose"]),
        &env,
        &FileDefaults::default(),
        true,
    )
    .expect("flags win in ci");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    let got =
        parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).expect("non-ci reads env");
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
}

fn slots(n: usize) -> dx_adopt::defaults::BuildSlots {
    dx_adopt::defaults::BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"))
}

fn slot_files(committed: Option<usize>, local: Option<usize>) -> FileDefaults {
    let committed = FileDefaults {
        build_slots: committed.map(slots),
        ..FileDefaults::default()
    };
    let local = FileDefaults {
        build_slots: local.map(slots),
        ..FileDefaults::default()
    };
    dx_adopt::defaults::merge_defaults(committed, local)
}

fn resolved_slots(
    words: &[&str],
    env: &[(&str, &str)],
    file: &FileDefaults,
) -> dx_adopt::defaults::BuildSlots {
    parse_with(&strings(words), &env_of(env), file)
        .expect("parse")
        .build_slots
}

#[test]
fn build_slots_default_is_auto() {
    assert_eq!(
        resolved_slots(&["build"], &[], &FileDefaults::default()),
        dx_adopt::defaults::BuildSlots::Auto
    );
}

#[test]
fn build_slots_flag_beats_env() {
    let env = [("DX_BUILD_SLOTS", "3")];
    let file = FileDefaults::default();
    assert_eq!(
        resolved_slots(&["build", "--build-slots=2"], &env, &file),
        slots(2)
    );
    assert_eq!(
        resolved_slots(&["build", "--build-slots", "off"], &env, &file),
        dx_adopt::defaults::BuildSlots::Off
    );
}

#[test]
fn build_slots_env_beats_local_file() {
    let file = slot_files(None, Some(4));
    assert_eq!(
        resolved_slots(&["build"], &[("DX_BUILD_SLOTS", "3")], &file),
        slots(3)
    );
}

#[test]
fn build_slots_local_file_beats_committed_file() {
    let file = slot_files(Some(5), Some(4));
    assert_eq!(resolved_slots(&["build"], &[], &file), slots(4));
    let file = slot_files(Some(5), None);
    assert_eq!(resolved_slots(&["build"], &[], &file), slots(5));
}

#[test]
fn build_slots_empty_env_is_unset() {
    let file = slot_files(Some(5), None);
    assert_eq!(
        resolved_slots(&["build"], &[("DX_BUILD_SLOTS", "")], &file),
        slots(5)
    );
}

#[test]
fn ci_ignores_build_slots_env_but_keeps_flag_and_committed_file() {
    use super::parse_with_ci;
    let env = env_of(&[("DX_BUILD_SLOTS", "off")]);
    let committed = slot_files(Some(5), None);
    let got = parse_with_ci(&strings(&["build"]), &env, &committed, true).expect("ci");
    assert_eq!(got.build_slots, slots(5));
    let got = parse_with_ci(
        &strings(&["build", "--build-slots=2"]),
        &env,
        &committed,
        true,
    )
    .expect("ci flag");
    assert_eq!(got.build_slots, slots(2));
    let got = parse_with_ci(&strings(&["build"]), &env, &FileDefaults::default(), true)
        .expect("ci default");
    assert_eq!(got.build_slots, dx_adopt::defaults::BuildSlots::Auto);
}

#[test]
fn bad_build_slots_are_usage_errors() {
    use super::super::ArgsError;
    let file = FileDefaults::default();
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
        let error = parse_with(&strings(&words), &env_of(&env), &file).expect_err("bad");
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
    let file = FileDefaults::default();
    assert_eq!(
        resolved_slots(&["build", "--build-slots=2"], &[], &file),
        slots(2)
    );
    assert_eq!(
        resolved_slots(&["status", "--build-slots=off"], &[], &file),
        dx_adopt::defaults::BuildSlots::Off
    );
    assert_eq!(
        resolved_slots(&["lint", "--build-slots", "3", "//..."], &[], &file),
        slots(3)
    );
}
