use super::super::{ArgsError, Command, FileDefaults, OperationMode};
use super::parse_with;
use crate::test_support::strings;
use dx_adopt::defaults::{
    config_key, parse_bool, parse_file_text, BOOL_SPELLINGS, DX_DRY_RUN_ENV, DX_OUTPUT_ENV,
    DX_QUIET_ENV, DX_VERBOSE_ENV, DX_WORKSPACE_ENV, ENV_DEFAULTS,
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
fn baseline_selection_flows_from_the_file() {
    let mut file = FileDefaults::default();
    file.baseline = Some("quality-baseline.json".to_owned());
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &file).expect("file baseline");
    assert_eq!(got.baseline, Some("quality-baseline.json".to_owned()));
    let got = parse_with(&strings(&["lint"]), &env_of(&[]), &FileDefaults::default())
        .expect("no baseline");
    assert_eq!(got.baseline, None);
}

#[test]
fn quiet_flows_into_text_mode() {    let env = env_of(&[(DX_QUIET_ENV, "true")]);
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
    assert_startup_outcome(&out, "invalid_arguments", 2);
    assert!(
        err.contains("DX_DRY_RUN=\"tru\"") && err.contains(BOOL_SPELLINGS),
        "the diagnostic names the variable and the spellings: {err}"
    );
    assert!(err.contains("usage: dx"), "the usage line follows: {err}");
    std::fs::write(&config, "[dx]\ndry_run = true\nqiet = true\n").expect("typo");
    let (code, out, err) = run(&["version", "--output=json"], &[]);
    assert_eq!(code, Some(2), "a misspelled key is a usage error: {err}");
    assert_startup_outcome(&out, "invalid_defaults", 2);
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
    std::fs::write(
        scratch.path().join(".dx/config.toml"),
        format!("[dx]\noutput = \"{output}\"\n"),
    )
    .expect("defaults");
    scratch
}

#[test]
fn help_works_with_a_malformed_config_but_operations_fail_closed() {
    let scratch = dx_test_scratch::scratch("dx-startup-malformed-help-");
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join(".dx/config.toml"), "not toml = [").expect("bad config");
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
    let (code, out, err) = run_dx(root, &["version", "--dry-run"], &[]);
    assert_eq!(code, Some(2), "an operation still fails closed: {err}");
    assert!(out.is_empty(), "no events for a usage error: {out}");
    assert!(
        err.contains("config.toml"),
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
        err.contains("usage: dx"),
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
    let (code, out, err) = run_dx(a.path(), &["version", "--dry-run"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the start directory is json: {out}"
    );
    let (code, out, err) = run_dx(
        a.path(),
        &["version", "--dry-run", "--workspace", &b_dir],
        &[],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("would report version"),
        "the flag workspace supplies text defaults: {out}"
    );
    let (code, out, err) = run_dx(
        a.path(),
        &["version", "--dry-run"],
        &[(DX_WORKSPACE_ENV, &b_dir)],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("would report version"),
        "the environment workspace supplies text defaults: {out}"
    );
    let nested = a.path().join("sub/dir");
    std::fs::create_dir_all(&nested).expect("nested");
    let (code, out, err) = run_dx(
        &nested,
        &["version", "--dry-run", "--workspace", &b_dir],
        &[],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("would report version"),
        "a nested start still reads the flag workspace: {out}"
    );
}

#[test]
fn bazel_run_start_selects_the_workspace_root() {
    let a = workspace_with_output("dx-startup-bwd-a-", "json");
    let b = workspace_with_output("dx-startup-bwd-b-", "text");
    let b_dir = b.path().to_string_lossy().into_owned();
    let (code, out, err) = run_dx(
        a.path(),
        &["version", "--dry-run"],
        &[("BUILD_WORKSPACE_DIRECTORY", &b_dir)],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("would report version"),
        "the Bazel start supplies text defaults: {out}"
    );
}

#[test]
fn new_and_completion_run_without_a_workspace() {
    let scratch = dx_test_scratch::scratch("dx-startup-outside-");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["new", "rust", "demo"], &[]);
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
fn config_directed_workspace_loads_the_target_defaults_once() {
    let b = workspace_with_output("dx-startup-redir-b-", "json");
    let b_dir = b.path().to_string_lossy().into_owned();
    let a = dx_test_scratch::scratch("dx-startup-redir-a-");
    std::fs::write(a.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(a.path().join(".dx")).expect("dx");
    std::fs::write(
        a.path().join(".dx/config.toml"),
        format!("[dx]\nworkspace = \"{b_dir}\"\n"),
    )
    .expect("redirect");
    let (code, out, err) = run_dx(a.path(), &["version", "--dry-run"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the redirect target supplies json defaults: {out}"
    );
    let a_dir = a.path().to_string_lossy().into_owned();
    std::fs::write(
        b.path().join(".dx/config.toml"),
        format!("[dx]\noutput = \"text\"\nworkspace = \"{a_dir}\"\n"),
    )
    .expect("cycle");
    let (code, out, err) = run_dx(a.path(), &["version", "--dry-run"], &[]);
    assert_eq!(code, Some(0), "a redirect cycle still terminates: {err}");
    assert!(
        out.contains("would report version"),
        "the frozen target supplies text defaults: {out}"
    );
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
    assert_eq!(
        parse_with(
            &strings(&["fix", "--apply"]),
            &env_of(&[(DX_DRY_RUN_ENV, "1")]),
            &FileDefaults::default(),
        ),
        Err(ArgsError::ConflictingModes {
            first: "--dry-run",
            second: "--apply",
        })
    );
    let file = file_with(None, None, None, None, Some(true), None);
    assert_eq!(
        parse_with(&strings(&["fix", "--apply"]), &env_of(&[]), &file),
        Err(ArgsError::ConflictingModes {
            first: "--dry-run",
            second: "--apply",
        })
    );
}

#[test]
fn ci_ignores_preference_env_but_keeps_flags() {
    use super::parse_with_ci;
    let env = env_of(&[("DX_OUTPUT", "json"), ("DX_VERBOSE", "1")]);
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
