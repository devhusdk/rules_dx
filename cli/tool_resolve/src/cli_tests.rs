use super::*;

fn cli(label: &str, var: &str) -> Cli {
    Cli {
        label: label.to_owned(),
        var: var.to_owned(),
        bazel: PathBuf::from("bazel"),
        workspace: None,
        env_file: None,
        print: true,
        bazel_args: Vec::new(),
    }
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|&arg| arg.to_owned()).collect()
}

#[test]
fn bad_variable_names_fail_before_bazel_runs() {
    let cli = cli("@dx_tools//:gitleaks", "DX-BIN");
    let error = run(cli, None, None).unwrap_err();
    assert!(
        error.to_string().contains("DX-BIN"),
        "the diagnostic must name the variable: {error}"
    );
}

#[test]
fn bad_labels_fail_before_bazel_runs() {
    let cli = cli("not a label", "DX_GITLEAKS_BIN");
    assert!(run(cli, None, None).is_err());
}

#[test]
fn missing_github_env_without_print_or_file_is_a_usage_error() {
    let mut cli = cli("@dx_tools//:gitleaks", "DX_GITLEAKS_BIN");
    cli.print = false;
    let error = run(cli, None, None).unwrap_err();
    match error {
        ResolveError::Usage { detail } => {
            assert!(detail.contains("GITHUB_ENV"), "{detail}");
        }
        other @ (ResolveError::CqueryFailed { .. }
        | ResolveError::NoArtifact { .. }
        | ResolveError::InfoFailed { .. }
        | ResolveError::MissingArtifact { .. }
        | ResolveError::MissingOutput { .. }
        | ResolveError::EnvWrite { .. }) => {
            panic!("a missing GITHUB_ENV must be a usage error: {other}")
        }
    }
}

#[test]
fn an_explicit_env_file_wins_over_github_env() {
    let target = env_target(
        Some(Path::new("explicit.env")),
        Some(OsStr::new("github.env")),
    )
    .unwrap();
    assert_eq!(target, PathBuf::from("explicit.env"));
    let target = env_target(None, Some(OsStr::new("github.env"))).unwrap();
    assert_eq!(target, PathBuf::from("github.env"));
}

#[test]
fn help_exits_zero() {
    assert_eq!(run_process(&args(&["--help"])), 0);
}

#[test]
fn a_missing_var_is_a_usage_error() {
    assert_eq!(
        run_process(&args(&["--label", "@dx_tools//:gitleaks", "--print"])),
        ErrorKind::Usage.exit_code()
    );
    assert_eq!(ErrorKind::Usage.exit_code(), 2);
}
