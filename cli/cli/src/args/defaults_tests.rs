use super::super::{Command, FileDefaults};
use super::parse_with;
use crate::test_support::strings;
use dx_adopt::defaults::{
    parse_bool, parse_file_text, BOOL_SPELLINGS, DX_DRY_RUN_ENV, DX_QUIET_ENV, DX_VERBOSE_ENV,
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
    dry_run: Option<bool>,
    fail_on: Option<&str>,
) -> FileDefaults {
    FileDefaults {
        workspace: workspace.map(ToString::to_string),
        output: output.map(ToString::to_string),
        verbose,
        color: None,
        quiet,
        dry_run,
        fail_on: fail_on.map(ToString::to_string),
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
    assert!(!got.dry_run);
    assert_eq!(got.output, OutputMode::Text { quiet: false });
    assert_eq!(got.fail_on, Threshold::Warning);
}

#[test]
fn env_supplies_workspace_output_and_bools() {
    let empty = FileDefaults::default();
    let env = env_of(&[
        ("DX_WORKSPACE", "/repo"),
        ("DX_OUTPUT", "json"),
        ("DX_VERBOSE", "yes"),
        ("DX_QUIET", "1"),
        ("DX_DRY_RUN", "on"),
        ("DX_FAIL_ON", "error"),
    ]);
    let got = parse_with(&strings(&["lint"]), &env, &empty).expect("env parse");
    assert_eq!(got.workspace, Some("/repo".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    assert!(got.quiet);
    assert!(got.dry_run);
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
        Some(true),
        Some("error"),
    );
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &file).expect("file parse");
    assert_eq!(got.workspace, Some("/file-ws".to_owned()));
    assert_eq!(got.output, OutputMode::Json);
    assert!(got.verbose);
    assert!(got.quiet);
    assert!(got.dry_run);
    assert_eq!(got.fail_on, Threshold::Error);
}

#[test]
fn precedence_is_flag_over_env_over_file() {
    let file = file_with(
        Some("/file"),
        Some("diff"),
        Some(true),
        Some(false),
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
    let file_bools = file_with(None, None, Some(true), None, None, None);
    let got = parse_with(&strings(&["lint", "--verbose"]), &env_of(&[]), &file_bools)
        .expect("flag bool wins");
    assert!(got.verbose);
    let env_true = env_of(&[("DX_VERBOSE", "1")]);
    let got = parse_with(
        &strings(&["lint"]),
        &env_true,
        &file_with(None, None, Some(false), None, None, None),
    )
    .expect("env bool wins");
    assert!(got.verbose);
    let env_false = env_of(&[("DX_VERBOSE", "0")]);
    let got = parse_with(
        &strings(&["lint"]),
        &env_false,
        &file_with(None, None, Some(true), None, None, None),
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
    let file = file_with(None, Some("yaml"), None, None, None, None);
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
    let file = file_with(None, None, Some(true), Some(true), Some(true), None);
    let off = |flag: &str| {
        parse_with(&strings(&["lint", flag]), &env_of(&[]), &file)
            .unwrap_or_else(|error| panic!("{flag}: {error:?}"))
    };
    assert!(!off("--dry-run=false").dry_run, "flag false beats the file");
    assert!(!off("--quiet=false").quiet, "flag false beats the file");
    assert!(!off("--verbose=false").verbose, "flag false beats the file");
    assert!(!off("-v=false").verbose, "the short form takes a value too");
    let on = |flag: &str| {
        parse_with(&strings(&["lint", flag]), &env_of(&[]), &file)
            .unwrap_or_else(|error| panic!("{flag}: {error:?}"))
    };
    assert!(on("--dry-run").dry_run, "the bare form stays true");
    assert!(on("--quiet").quiet, "the bare form stays true");
    assert!(on("--verbose").verbose, "the bare form stays true");
    assert!(on("-v").verbose, "the bare short form stays true");
    assert!(on("--dry-run=true").dry_run, "an explicit true is true");
    let env = env_of(&[(DX_VERBOSE_ENV, "1")]);
    let got = parse_with(&strings(&["lint", "--verbose=false"]), &env, &file)
        .expect("flag false beats the environment");
    assert!(!got.verbose, "the flag beats the environment");
    let got = parse_with(
        &strings(&["lint", "--dry-run", "--dry-run=false"]),
        &env_of(&[]),
        &file,
    )
    .expect("last one wins");
    assert!(!got.dry_run, "the last occurrence wins");
    for (env_name, index) in [(DX_DRY_RUN_ENV, 0), (DX_QUIET_ENV, 1), (DX_VERBOSE_ENV, 2)] {
        for value in ["0", "false", "no", "n", "off"] {
            let env = owned_env(&[(env_name, value)]);
            let got = parse_with(&strings(&["lint"]), &env, &file)
                .unwrap_or_else(|error| panic!("{env_name}={value}: {error:?}"));
            let flags = [got.dry_run, got.quiet, got.verbose];
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
        for value in ["1", "true", "yes", "y", "on"] {
            let env = owned_env(&[(env_name, value)]);
            let got = parse_with(&strings(&["lint"]), &env, &FileDefaults::default())
                .unwrap_or_else(|error| panic!("{env_name}={value}: {error:?}"));
            assert_eq!(
                [got.dry_run, got.quiet, got.verbose][index],
                true,
                "{env_name}={value} turns the default on"
            );
        }
    }
    for word in ["yes", "off", "n", "ON", "N", "1", "true", "0"] {
        let env = owned_env(&[(DX_DRY_RUN_ENV, word)]);
        let got = parse_with(&strings(&["lint"]), &env, &FileDefaults::default())
            .unwrap_or_else(|error| panic!("DX_DRY_RUN={word}: {error:?}"));
        assert_eq!(got.dry_run, parse_bool(word) == Some(true), "{word}");
    }
}

#[test]
fn a_boolean_value_outside_the_spellings_is_a_usage_error() {
    use super::super::ArgsError;
    for (env_name, value) in [
        (DX_DRY_RUN_ENV, "tru"),
        (DX_QUIET_ENV, "ture"),
        (DX_VERBOSE_ENV, "2"),
        (DX_DRY_RUN_ENV, "maybe"),
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
    for value in ["", "true", "on", "no", "off", "0", "1"] {
        let env = owned_env(&[(DX_DRY_RUN_ENV, value)]);
        assert!(
            parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).is_ok(),
            "DX_DRY_RUN={value:?} must parse"
        );
    }
}

#[test]
fn boolean_precedence_runs_cli_then_env_then_dx_table_then_top_level() {
    let file = parse_file_text(
        "dry_run = false\nquiet = false\nverbose = false\n[dx]\ndry_run = false\nquiet = false\nverbose = false\n",
    )
    .expect("both layers");
    let table_only = parse_file_text("[dx]\nquiet = true\n").expect("dx table");
    let top_only = parse_file_text("quiet = true\n").expect("top level");
    assert_eq!(file.dry_run, Some(false), "[dx] wins over the top level");
    assert_eq!(table_only.quiet, Some(true));
    assert_eq!(top_only.quiet, Some(true));
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &FileDefaults::default())
        .expect("the built-in default is off");
    assert!(!got.quiet && !got.dry_run && !got.verbose);
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &top_only).expect("top level");
    assert!(got.quiet, "the top level beats the built-in default");
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &table_only).expect("dx table");
    assert!(got.quiet, "[dx] beats the top level");
    let env = env_of(&[(DX_QUIET_ENV, "0")]);
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
    let config = root.join(".dx/config.toml");
    std::fs::write(
        &config,
        "[dx]\ndry_run = true\nquiet = true\nverbose = true\n",
    )
    .expect("defaults");
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
    let (code, out, err) = run(&["version", "--output=json"], &[]);
    assert_eq!(code, Some(0), "the config file defaults apply: {err}");
    assert!(
        out.contains("\"dry_run\":true"),
        "the config file turns dry_run on: {out}"
    );
    assert!(
        err.contains("dx invocation parsed"),
        "the config file turns verbose on: {err}"
    );
    let (code, out, err) = run(&["version"], &[]);
    assert_eq!(code, Some(0), "quiet text mode still succeeds: {err}");
    assert!(out.is_empty(), "the config file turns quiet on: {out}");
    let (code, out, err) = run(&["version", "--dry-run=false", "--output=json"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("\"dry_run\":false"),
        "an explicit false turns the inherited dry_run off: {out}"
    );
    let (code, out, err) = run(&["version", "--quiet=false"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("would report version"),
        "an explicit false turns the inherited quiet off: {out}"
    );
    for flag in ["--verbose=false", "-v=false"] {
        let (code, _, err) = run(&["version", flag], &[]);
        assert_eq!(code, Some(0), "{flag}: {err}");
        assert!(
            !err.contains("dx invocation parsed"),
            "{flag} turns the inherited verbose off: {err}"
        );
    }
    let (code, out, err) = run(&["version", "--output=json"], &[(DX_DRY_RUN_ENV, "1")]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("\"dry_run\":true"),
        "the environment turns dry_run on: {out}"
    );
    let (code, out, err) = run(
        &["version", "--output=json", "--dry-run=false"],
        &[(DX_DRY_RUN_ENV, "1")],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("\"dry_run\":false"),
        "the flag beats the environment: {out}"
    );
    let (code, out, err) = run(&["version", "--output=json"], &[(DX_DRY_RUN_ENV, "tru")]);
    assert_eq!(code, Some(2), "a typo is a usage error: {err}");
    assert!(out.is_empty(), "no events for a usage error: {out}");
    assert!(
        err.contains("DX_DRY_RUN=\"tru\"") && err.contains(BOOL_SPELLINGS),
        "the diagnostic names the variable and the spellings: {err}"
    );
    assert!(err.contains("usage: dx"), "the usage line follows: {err}");
    std::fs::write(&config, "[dx]\ndry_run = true\nqiet = true\n").expect("typo");
    let (code, out, err) = run(&["version", "--output=json"], &[]);
    assert_eq!(code, Some(2), "a misspelled key is a usage error: {err}");
    assert!(out.is_empty(), "no events for a usage error: {out}");
    assert!(
        err.contains("unknown field `qiet`") && err.contains("config.toml"),
        "the diagnostic names the file and the key: {err}"
    );
    std::fs::write(&config, "[dx]\nquiet = true\n").expect("valid");
    let (code, out, err) = run(&["version", "--output=json"], &[(DX_QUIET_ENV, "1")]);
    assert_eq!(code, Some(0), "a documented spelling still parses: {err}");
    assert!(
        out.contains("\"dry_run\":false") && out.contains("\"command_finished\""),
        "{out}"
    );
    let (code, _, err) = run(&["version", "--dry-run=maybe"], &[]);
    assert_eq!(code, Some(2), "a bad flag value stays a usage error");
    assert!(
        err.contains("invalid value 'maybe'"),
        "a bad flag value stays a clap usage error: {err}"
    );
    scratch.close().expect("cleanup");
}

const SAMPLE_MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\n";

fn dx_binary() -> std::path::PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx")
}
