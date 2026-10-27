use super::super::{Command, FileDefaults, OperationMode};
use super::parse_with;
use crate::test_support::strings;
use dx_adopt::defaults::{
    config_key, parse_bool, parse_file_text, BOOL_SPELLINGS, DX_OUTPUT_ENV, DX_QUIET_ENV,
    DX_VERBOSE_ENV, DX_WORKSPACE_ENV, ENV_DEFAULTS,
};
use dx_output::{ColorMode, OutputMode, Threshold};

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
fn the_dx_process_turns_injected_defaults_on_and_off() {
    let scratch = dx_test_scratch::scratch("dx-defaults-process-");
    let root = scratch.path().to_path_buf();
    std::fs::create_dir(root.join(".dx")).expect("dx");
    std::fs::write(root.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
    let config = root.join("dx.toml");
    std::fs::write(&config, "[dx]\nquiet = true\nverbose = true\n").expect("defaults");
    let run = |words: &[&str], envs: &[(&str, &str)]| {
        let mut command = assert_cmd::Command::new(dx_binary());
        command.current_dir(&root);
        for (name, value) in envs {
            command.env(name, value);
        }
        let output = command.args(words).output().expect("dx runs");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let (code, _, err) = run(&["status", "--output=json"], &[]);
    assert_eq!(code, Some(0), "the config file defaults apply: {err}");
    assert!(
        err.contains("dx invocation parsed"),
        "the config file turns verbose on: {err}"
    );
    let (code, out, err) = run(&["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("COMPLETE"), "{out}");
    for flag in ["--verbose=false", "-v=false"] {
        let (code, _, err) = run(&["status", flag], &[]);
        assert_eq!(code, Some(0), "{flag}: {err}");
        assert!(
            !err.contains("dx invocation parsed"),
            "{flag} turns the inherited verbose off: {err}"
        );
    }
    let (code, _, err) = run(&["status", "--output=json"], &[(DX_QUIET_ENV, "tru")]);
    assert_eq!(code, Some(2), "a typo is a usage error: {err}");
    assert!(
        err.contains("DX_QUIET=\"tru\"") && err.contains(BOOL_SPELLINGS),
        "the diagnostic names the variable and the spellings: {err}"
    );
    assert!(err.contains("Usage: dx"), "the usage line follows: {err}");
    std::fs::write(&config, "[dx]\nqiet = true\n").expect("typo");
    let (code, out, err) = run(&["status", "--output=json"], &[]);
    assert_eq!(code, Some(2), "a misspelled key is a usage error: {err}");
    assert_startup_outcome(&out, "invalid_defaults", 2);
    assert!(
        err.contains("dx.toml"),
        "the diagnostic names the file: {err}"
    );
    std::fs::write(&config, "[dx]\nquiet = true\n").expect("valid");
    let (code, out, err) = run(&["status", "--output=json"], &[(DX_QUIET_ENV, "true")]);
    assert_eq!(code, Some(0), "a documented spelling still parses: {err}");
    assert!(out.contains("\"command_finished\""), "{out}");
    let (code, _, err) = run(&["status", "--quiet=maybe"], &[]);
    assert_eq!(code, Some(2), "a bad flag value stays a usage error");
    assert!(
        err.contains("invalid value 'maybe'"),
        "a bad flag value stays a clap usage error: {err}"
    );
    scratch.close().expect("cleanup");
}

const SAMPLE_MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

fn dx_binary() -> std::path::PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx")
}

/// Runs the built binary in `dir` with extra environment, returning exit code,
// stdout, and stderr.
fn run_dx(
    dir: &std::path::Path,
    words: &[&str],
    envs: &[(&str, &str)],
) -> (Option<i32>, String, String) {
    let mut command = assert_cmd::Command::new(dx_binary());
    command.current_dir(dir);
    for (name, value) in envs {
        command.env(name, value);
    }
    let output = command.args(words).output().expect("dx runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Checks one pre-execution failure stream: an `error` event naming the
/// machine-readable code, then the terminal `command_finished` with the
/// process exit and incomplete results, and nothing else on stdout.
fn assert_startup_outcome(out: &str, code: &str, exit: i32) {
    let mut lines = out.lines();
    let first: serde_json::Value = serde_json::from_str(lines.next().expect("an error event"))
        .expect("every stdout line is json");
    assert_eq!(first["event"], "error", "{out}");
    assert_eq!(first["code"], code, "{out}");
    assert!(
        !first["message"].as_str().unwrap_or_default().is_empty(),
        "{out}"
    );
    let second: serde_json::Value = serde_json::from_str(lines.next().expect("a finished event"))
        .expect("every stdout line is json");
    assert_eq!(second["event"], "command_finished", "{out}");
    assert_eq!(second["exit_code"], exit, "{out}");
    assert_eq!(second["results_complete"], false, "{out}");
    assert!(
        lines.next().is_none(),
        "the terminal event ends the stream: {out}"
    );
}

fn workspace_with_output(name: &str, output: &str) -> dx_test_scratch::TempDir {
    let scratch = dx_test_scratch::scratch(name);
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(
        scratch.path().join("dx.toml"),
        format!("[dx]\noutput = \"{output}\"\n"),
    )
    .expect("defaults");
    scratch
}

#[test]
fn unsupported_file_defaults_do_not_break_completion() {
    let scratch = workspace_with_output("dx-unsupported-defaults-", "json");
    std::fs::write(
        scratch.path().join("dx.toml"),
        "[dx]\noutput = \"json\"\nfail-on = \"error\"\n",
    )
    .expect("defaults");
    let (code, out, err) = run_dx(scratch.path(), &["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("complete"), "{out}");
    scratch.close().expect("cleanup");
}

#[test]
fn help_works_with_a_malformed_config_but_operations_fail_closed() {
    let scratch = dx_test_scratch::scratch("dx-startup-malformed-help-");
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join("dx.toml"), "not toml = [").expect("bad config");
    let root = scratch.path();
    let (code, out, _) = run_dx(root, &["--help"], &[]);
    assert_eq!(code, Some(0), "--help works with a malformed config");
    assert!(out.contains("Run Bazel workflows"), "{out}");
    let (code, out, _) = run_dx(root, &["lint", "--help"], &[]);
    assert_eq!(code, Some(0), "command help works with a malformed config");
    assert!(out.contains("dx lint"), "{out}");
    let (code, out, _) = run_dx(root, &["help", "lint"], &[]);
    assert_eq!(code, Some(0), "the help verb works with a malformed config");
    assert!(out.contains("dx lint"), "{out}");
    let (code, out, err) = run_dx(root, &["status"], &[]);
    assert_eq!(code, Some(2), "an operation still fails closed: {err}");
    assert!(out.is_empty(), "no events for a usage error: {out}");
    assert!(
        err.contains("dx.toml"),
        "the diagnostic names the file: {err}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn startup_argument_failures_report_structured_json() {
    let scratch = dx_test_scratch::scratch("dx-startup-json-usage-");
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["lint", ":oops", "--output=json"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    assert!(
        err.contains("Usage: dx"),
        "stderr keeps the usage line: {err}"
    );
    let (code, out, err) = run_dx(root, &["lint", ":oops"], &[(DX_OUTPUT_ENV, "json")]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let (code, out, _) = run_dx(root, &["lint", ":oops"], &[]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "text mode stays event-free: {out}");
    let (code, out, _) = run_dx(root, &["lint", ":oops", "--output=xml"], &[]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "a junk output value stays text-only: {out}");
    for short in [
        &["lint", ":oops", "-o", "json"][..],
        &["lint", ":oops", "-ojson"],
        &["lint", ":oops", "-qojson"],
        &["lint", ":oops", "-vo", "json"],
    ] {
        let (code, out, err) = run_dx(root, short, &[]);
        assert_eq!(code, Some(2), "{err}");
        assert_startup_outcome(&out, "invalid_arguments", 2);
    }
    let (code, out, err) = run_dx(root, &["lint", ":oops", "-o", "json"], &[("CI", "true")]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let (code, out, err) = run_dx(root, &["lint", ":oops", "-qojson"], &[("CI", "true")]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let (code, out, _) = run_dx(
        root,
        &["lint", ":oops", "-o", "text"],
        &[(DX_OUTPUT_ENV, "json")],
    );
    assert_eq!(code, Some(2));
    assert!(
        out.is_empty(),
        "a short text flag beats the environment: {out}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn startup_file_and_workspace_failures_report_structured_json() {
    let scratch = workspace_with_output("dx-startup-json-file-", "json");
    let (code, out, err) = run_dx(scratch.path(), &["lint", ":oops"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let missing = scratch
        .path()
        .join("no-such-dir")
        .to_string_lossy()
        .into_owned();
    let (code, out, err) = run_dx(scratch.path(), &["version", "--workspace", &missing], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "unresolved_workspace", 2);
    assert!(err.contains("cannot resolve workspace"), "{err}");
}

#[test]
fn explicit_workspace_uses_the_target_workspace_defaults() {
    let a = workspace_with_output("dx-startup-ws-a-", "json");
    let b = workspace_with_output("dx-startup-ws-b-", "text");
    let b_dir = b.path().to_string_lossy().into_owned();
    let (code, out, err) = run_dx(a.path(), &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the start directory is json: {out}"
    );
    let (code, out, err) = run_dx(a.path(), &["status", "--workspace", &b_dir], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the flag workspace supplies text defaults: {out}"
    );
    let (code, out, err) = run_dx(a.path(), &["status"], &[(DX_WORKSPACE_ENV, &b_dir)]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the environment workspace supplies text defaults: {out}"
    );
    let nested = a.path().join("sub/dir");
    std::fs::create_dir_all(&nested).expect("nested");
    let (code, out, err) = run_dx(&nested, &["status", "--workspace", &b_dir], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "a nested start still reads the flag workspace: {out}"
    );
}

#[test]
fn ci_ignores_the_environment_workspace_for_defaults() {
    let a = workspace_with_output("dx-startup-ci-a-", "json");
    let b = workspace_with_output("dx-startup-ci-b-", "text");
    let a_dir = a.path().to_string_lossy().into_owned();
    let ci = (("CI"), "true");
    let (code, out, err) = run_dx(b.path(), &["status"], &[ci, (DX_WORKSPACE_ENV, &a_dir)]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: ") && !out.contains("command_started"),
        "the ignored environment workspace leaves b's text defaults: {out}"
    );
    std::fs::write(a.path().join("dx.toml"), "[dx\noutput = \n").expect("malformed");
    let (code, out, err) = run_dx(b.path(), &["status"], &[ci, (DX_WORKSPACE_ENV, &a_dir)]);
    assert_eq!(
        code,
        Some(0),
        "a malformed ignored workspace is not read: {err}"
    );
    assert!(out.contains("toolchain: "), "{out}");
    let redirect = format!("[dx]\noutput = \"json\"\nworkspace = \"{a_dir}\"\n");
    let c = workspace_with_output("dx-startup-ci-c-", "json");
    std::fs::write(c.path().join("dx.toml"), redirect).expect("redirect");
    std::fs::write(a.path().join("dx.toml"), "[dx\noutput = \n").expect("malformed");
    let (code, _, err) = run_dx(b.path(), &["status"], &[ci, (DX_WORKSPACE_ENV, &a_dir)]);
    assert_eq!(code, Some(0), "{err}");
    let (code, _, err) = run_dx(
        b.path(),
        &["status", "--workspace", &a_dir],
        &[ci, (DX_WORKSPACE_ENV, &a_dir)],
    );
    assert_ne!(code, Some(0), "an explicit flag still selects a: {err}");
    std::fs::write(a.path().join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("json");
    let (code, out, err) = run_dx(
        b.path(),
        &["status", "--workspace", &a_dir],
        &[ci, (DX_WORKSPACE_ENV, &a_dir)],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("command_started"), "the flag selects a: {out}");
}

#[test]
fn repeated_workspace_flags_load_the_last_workspace_defaults() {
    let a = workspace_with_output("dx-startup-last-a-", "json");
    let b = workspace_with_output("dx-startup-last-b-", "text");
    let a_dir = a.path().to_string_lossy().into_owned();
    let b_dir = b.path().to_string_lossy().into_owned();
    let inline_a = format!("--workspace={a_dir}");
    let inline_b = format!("--workspace={b_dir}");
    let cases: Vec<Vec<&str>> = vec![
        vec!["status", "--workspace", &a_dir, "--workspace", &b_dir],
        vec!["status", &inline_a, &inline_b],
        vec!["status", "--workspace", &a_dir, &inline_b],
        vec![
            "status",
            "--output",
            "text",
            "--workspace",
            &a_dir,
            "--workspace",
            &b_dir,
        ],
    ];
    for words in cases {
        let (code, out, err) = run_dx(a.path(), &words, &[]);
        assert_eq!(code, Some(0), "{words:?}: {err}");
        assert!(
            out.contains("toolchain: ") && !out.contains("command_started"),
            "{words:?}: the last workspace supplies text defaults: {out}"
        );
    }
    let (code, out, err) = run_dx(
        b.path(),
        &["status", "--workspace", &b_dir, "--workspace", &a_dir],
        &[],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the last workspace supplies json defaults: {out}"
    );
}

#[test]
fn overridden_workspace_with_malformed_defaults_does_not_block_the_last() {
    let a = workspace_with_output("dx-startup-bad-a-", "json");
    std::fs::write(a.path().join("dx.toml"), "[dx\noutput = \n").expect("malformed");
    let b = workspace_with_output("dx-startup-bad-b-", "text");
    let a_dir = a.path().to_string_lossy().into_owned();
    let b_dir = b.path().to_string_lossy().into_owned();
    let inline_a = format!("--workspace={a_dir}");
    let inline_b = format!("--workspace={b_dir}");
    let cases: Vec<Vec<&str>> = vec![
        vec!["status", "--workspace", &a_dir, "--workspace", &b_dir],
        vec!["status", &inline_a, &inline_b],
    ];
    for words in cases {
        let (code, out, err) = run_dx(a.path(), &words, &[]);
        assert_eq!(code, Some(0), "{words:?}: {err}");
        assert!(out.contains("toolchain: "), "{words:?}: {out}");
    }
    let (code, _, err) = run_dx(
        b.path(),
        &["status", "--workspace", &b_dir, "--workspace", &a_dir],
        &[],
    );
    assert_ne!(code, Some(0), "the malformed last workspace fails: {err}");
}

#[test]
fn bazel_run_start_selects_the_workspace_root() {
    let a = workspace_with_output("dx-startup-bwd-a-", "json");
    let b = workspace_with_output("dx-startup-bwd-b-", "text");
    let b_dir = b.path().to_string_lossy().into_owned();
    let (code, out, err) = run_dx(
        a.path(),
        &["status"],
        &[("BUILD_WORKSPACE_DIRECTORY", &b_dir)],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the Bazel start supplies text defaults: {out}"
    );
}

#[test]
fn bazel_run_here_uses_the_working_directory() {
    let ws = workspace_with_output("dx-startup-bwd-here-ws-", "json");
    let runfiles = dx_test_scratch::scratch("dx-startup-bwd-here-runfiles-");
    let outside = dx_test_scratch::scratch("dx-startup-bwd-here-outside-");
    let ws_dir = ws.path().to_string_lossy().into_owned();
    let outside_dir = outside.path().to_string_lossy().into_owned();
    let canonical_outside = outside.path().canonicalize().expect("canonical outside");
    let canonical_runfiles = runfiles.path().canonicalize().expect("canonical runfiles");
    let (code, out, err) = run_dx(
        runfiles.path(),
        &["lint", "--here", "--output=json"],
        &[
            ("BUILD_WORKSPACE_DIRECTORY", &ws_dir),
            ("BUILD_WORKING_DIRECTORY", &outside_dir),
        ],
    );
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_scope", 2);
    let canonical_outside = canonical_outside.display().to_string();
    let canonical_runfiles = canonical_runfiles.display().to_string();
    assert!(
        out.contains(&canonical_outside),
        "names the working directory: {out}"
    );
    assert!(
        !out.contains(&canonical_runfiles),
        "ignores the runfiles cwd: {out}"
    );
    let (code, out, err) = run_dx(
        runfiles.path(),
        &["lint", "--here", "--output=json"],
        &[
            ("BUILD_WORKSPACE_DIRECTORY", &ws_dir),
            ("BUILD_WORKING_DIRECTORY", "relative/dir"),
        ],
    );
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_scope", 2);
    assert!(
        out.contains(&canonical_runfiles),
        "a relative value falls back to the cwd: {out}"
    );
}

#[test]
fn bazel_run_relative_workspace_uses_the_working_directory() {
    let a = workspace_with_output("dx-startup-bwd-rel-a-", "json");
    let b = workspace_with_output("dx-startup-bwd-rel-b-", "text");
    let runfiles = dx_test_scratch::scratch("dx-startup-bwd-rel-runfiles-");
    let a_dir = a.path().to_string_lossy().into_owned();
    let b_parent = b
        .path()
        .parent()
        .expect("b parent")
        .to_string_lossy()
        .into_owned();
    let b_name = b
        .path()
        .file_name()
        .expect("b name")
        .to_string_lossy()
        .into_owned();
    let bazel_env = [
        ("BUILD_WORKSPACE_DIRECTORY", a_dir.as_str()),
        ("BUILD_WORKING_DIRECTORY", b_parent.as_str()),
    ];
    let flag = format!("--workspace={b_name}");
    let (code, out, err) = run_dx(runfiles.path(), &["status", &flag], &bazel_env);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the flag starts from the working directory: {out}"
    );
    let env = [
        bazel_env[0],
        bazel_env[1],
        ("DX_WORKSPACE", b_name.as_str()),
    ];
    let (code, out, err) = run_dx(runfiles.path(), &["status"], &env);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the environment starts from the working directory: {out}"
    );
    let inner = a.path().join("inner");
    std::fs::create_dir_all(inner.join(".dx")).expect("inner dx");
    std::fs::write(inner.join("MODULE.bazel"), SAMPLE_MODULE).expect("inner module");
    std::fs::write(inner.join(".dx/version"), "0.0.0\n").expect("inner pin");
    std::fs::write(inner.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("inner defaults");
    std::fs::write(
        a.path().join("dx.toml"),
        "[dx]\noutput = \"json\"\nworkspace = \"inner\"\n",
    )
    .expect("redirect");
    let (code, out, err) = run_dx(runfiles.path(), &["status"], &bazel_env);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the file key starts from the workspace start: {out}"
    );
}

#[test]
fn new_and_completion_run_without_a_workspace() {
    let scratch = dx_test_scratch::scratch("dx-startup-outside-");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["new", "--apply", "rust", "demo"], &[]);
    assert_eq!(
        code,
        Some(0),
        "standalone new works outside a workspace: {err}"
    );
    assert!(out.contains("demo/Cargo.toml"), "{out}");
    assert!(root.join("demo/Cargo.toml").exists());
    let (code, out, err) = run_dx(root, &["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "completion works outside a workspace: {err}");
    assert!(out.contains("COMPLETE"), "{out}");
    scratch.close().expect("cleanup");
}

#[test]
fn removed_new_spellings_exit_2_with_canonical_name() {
    let scratch = dx_test_scratch::scratch("dx-new-removed-");
    let root = scratch.path();
    for (spelling, canonical) in [
        ("cc", "cpp"),
        ("c#", "csharp"),
        ("c-sharp", "csharp"),
        ("f#", "fsharp"),
        ("f-sharp", "fsharp"),
    ] {
        for apply in [false, true] {
            let mut words = vec!["new"];
            if apply {
                words.push("--apply");
            }
            words.extend([spelling, "demo"]);
            let (code, out, err) = run_dx(root, &words, &[]);
            assert_eq!(code, Some(2), "{words:?}: {err}");
            assert!(out.is_empty(), "{words:?}: {out}");
            assert!(
                err.contains(&format!(
                    "language spelling for dx new was removed: {spelling} (use {canonical})"
                )),
                "{words:?}: {err}"
            );
            assert!(!root.join("demo").exists(), "{words:?} wrote demo");
        }
    }
    let (code, out, err) = run_dx(root, &["new", "--apply", "c", "demo"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("wrote demo/hello.c\n"), "{out}");
    assert!(root.join("demo/hello.c").is_file());
    assert!(!root.join("demo/hello.cc").exists());
    scratch.close().expect("cleanup");
}

#[test]
fn config_directed_workspace_loads_the_target_defaults_once() {
    let b = workspace_with_output("dx-startup-redir-b-", "json");
    let b_dir = b.path().to_string_lossy().into_owned();
    let a = dx_test_scratch::scratch("dx-startup-redir-a-");
    std::fs::write(a.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(a.path().join(".dx")).expect("dx");
    std::fs::write(a.path().join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(
        a.path().join("dx.toml"),
        format!("[dx]\nworkspace = \"{b_dir}\"\n"),
    )
    .expect("redirect");
    let (code, out, err) = run_dx(a.path(), &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the redirect target supplies json defaults: {out}"
    );
    let a_dir = a.path().to_string_lossy().into_owned();
    std::fs::write(
        b.path().join("dx.toml"),
        format!("[dx]\noutput = \"text\"\nworkspace = \"{a_dir}\"\n"),
    )
    .expect("cycle");
    let (code, out, err) = run_dx(a.path(), &["status"], &[]);
    assert_eq!(code, Some(0), "a redirect cycle still terminates: {err}");
    assert!(
        out.contains("toolchain: "),
        "the frozen target supplies text defaults: {out}"
    );
}

#[test]
fn relative_config_redirects_resolve_once_against_the_start_directory() {
    let outer = dx_test_scratch::scratch("dx-startup-once-");
    let root = outer.path();
    std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("outer");
    let repo = root.join("repo");
    std::fs::create_dir(&repo).expect("repo");
    std::fs::write(repo.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(repo.join(".dx")).expect(".dx");
    std::fs::write(repo.join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(repo.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("repo");
    let sub = repo.join("sub");
    std::fs::create_dir(&sub).expect("sub");
    std::fs::write(sub.join("dx.toml"), "[dx]\nworkspace = \"..\"\n").expect("sub");
    let (code, out, err) = run_dx(&sub, &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("command_started"), "repo json defaults: {out}");

    let repo_dir = repo.to_string_lossy().into_owned();
    std::fs::write(
        sub.join("dx.toml"),
        format!("[dx]\nworkspace = \"{repo_dir}\"\n"),
    )
    .expect("absolute");
    let (code, out, err) = run_dx(&sub, &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("command_started"), "absolute redirect: {out}");

    std::fs::write(sub.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("text");
    std::fs::write(repo.join("dx.toml"), "[dx]\nworkspace = \"nested\"\n").expect("hop");
    let nested = repo.join("nested");
    std::fs::create_dir(&nested).expect("nested");
    std::fs::write(nested.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(nested.join(".dx")).expect(".dx");
    std::fs::write(nested.join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(
        nested.join("dx.toml"),
        "[dx]\noutput = \"json\"\nworkspace = \"..\"\n",
    )
    .expect("nested");
    let (code, out, err) = run_dx(&repo, &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "a relative child redirect loads the child defaults, one hop only: {out}"
    );
    outer.close().expect("cleanup");
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

#[test]
fn bad_build_slots_exit_two_from_the_binary() {
    let scratch = workspace_with_output("dx-bad-build-slots-", "text");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["status", "--build-slots=0"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("--build-slots"), "{err}");
    let (code, _, err) = run_dx(root, &["status"], &[("DX_BUILD_SLOTS", "many")]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("DX_BUILD_SLOTS"), "{err}");
}

#[test]
fn removed_dry_run_is_rejected_from_every_source() {
    let scratch = workspace_with_output("dx-removed-dry-run-", "text");
    let root = scratch.path();
    for words in [
        vec!["lint", "--dry-run"],
        vec!["lint", "--dry-run=true"],
        vec!["lint", "--dry-run=false"],
        vec!["fix", "--dry-run", "--apply"],
    ] {
        let (code, out, err) = run_dx(root, &words, &[]);
        assert_eq!(code, Some(2), "{words:?}: {err}");
        assert!(out.is_empty(), "{words:?}: {out}");
        assert!(err.contains("--dry-run"), "{words:?}: {err}");
    }
    for value in ["true", "false", "1"] {
        let (code, _, err) = run_dx(root, &["lint"], &[("DX_DRY_RUN", value)]);
        assert_eq!(code, Some(2), "DX_DRY_RUN={value}: {err}");
        assert!(err.contains("DX_DRY_RUN was removed"), "{err}");
    }
    for key in ["dry_run", "dry-run"] {
        for value in ["true", "false"] {
            std::fs::write(root.join("dx.toml"), format!("[dx]\n{key} = {value}\n"))
                .expect("config");
            let (code, _, err) = run_dx(root, &["lint"], &[]);
            assert_eq!(code, Some(2), "{key}={value}: {err}");
            assert!(err.contains("was removed"), "{err}");
        }
    }
    scratch.close().expect("cleanup");
}

#[test]
fn log_level_config_keys_are_rejected() {
    let scratch = workspace_with_output("dx-log-level-config-", "text");
    let root = scratch.path();
    for key in ["log_level", "log-level"] {
        std::fs::write(root.join("dx.toml"), format!("[dx]\n{key} = \"debug\"\n")).expect("config");
        let (code, out, err) = run_dx(root, &["lint"], &[]);
        assert_eq!(code, Some(2), "{key}: {err}");
        assert!(out.is_empty(), "{key}: {out}");
        assert!(err.contains(key.split('-').next().unwrap()), "{key}: {err}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn log_level_flag_exits_two_before_running() {
    let scratch = workspace_with_output("dx-log-level-flag-", "text");
    let root = scratch.path();
    for words in [
        vec!["lint", "--log-level=debug"],
        vec!["lint", "-v", "--log-level", "trace"],
    ] {
        let (code, out, err) = run_dx(root, &words, &[]);
        assert_eq!(code, Some(2), "{words:?}: {err}");
        assert!(out.is_empty(), "{words:?}: {out}");
        assert!(err.contains("--log-level"), "{words:?}: {err}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn rust_log_does_not_change_dx_diagnostics() {
    let scratch = dx_test_scratch::scratch("dx-rust-log-");
    let root = scratch.path();
    let marker = "dx invocation parsed";
    for filter in ["trace", "info", "dx=trace", "off", "not a filter [", ""] {
        let (code, _, err) = run_dx(root, &["completion", "bash"], &[("RUST_LOG", filter)]);
        assert_eq!(code, Some(0), "RUST_LOG={filter}: {err}");
        assert!(
            !err.contains(marker),
            "default ignores RUST_LOG={filter}: {err}"
        );
        let (code, _, err) = run_dx(
            root,
            &["completion", "bash", "--verbose"],
            &[("RUST_LOG", filter)],
        );
        assert_eq!(code, Some(0), "RUST_LOG={filter}: {err}");
        assert!(
            err.contains(marker),
            "verbose ignores RUST_LOG={filter}: {err}"
        );
    }
    let (code, _, err) = run_dx(root, &["completion", "bash", "--verbose"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(err.contains(marker), "{err}");
    scratch.close().expect("cleanup");
}

#[test]
fn no_color_controls_auto_color_and_explicit_modes_win() {
    let scratch = dx_test_scratch::scratch("dx-no-color-");
    let root = scratch.path();
    let verbose = ["completion", "bash", "--verbose"];
    let escape = '\u{1b}';
    for (no_color, color, ansi) in [
        (None, "auto", false),
        (Some("1"), "auto", false),
        (Some(""), "auto", false),
        (Some("1"), "never", false),
        (Some(""), "always", true),
        (Some("1"), "always", true),
        (None, "always", true),
    ] {
        let color_arg = format!("--color={color}");
        let mut words = verbose.to_vec();
        words.push(&color_arg);
        let mut envs = Vec::new();
        if let Some(value) = no_color {
            envs.push(("NO_COLOR", value));
        }
        let (code, out, err) = run_dx(root, &words, &envs);
        assert_eq!(code, Some(0), "{no_color:?} {color}: {err}");
        assert_eq!(
            err.contains(escape),
            ansi,
            "NO_COLOR={no_color:?} --color={color}: {err:?}"
        );
        assert!(!out.contains(escape), "stdout stays plain: {out:?}");
    }
    scratch.close().expect("cleanup");
}
