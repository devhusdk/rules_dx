use super::super::{Command, FileDefaults};
use super::parse_with;
use crate::test_support::strings;
use dx_output::{ColorMode, OutputMode, Threshold};

fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
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
    let env = env_of(&[("DX_QUIET", "true")]);
    let got = parse_with(&strings(&["lint"]), &env, &FileDefaults::default()).expect("quiet env");
    assert!(got.quiet);
    assert_eq!(got.output, OutputMode::Text { quiet: true });
}
