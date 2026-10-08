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

#[test]
fn bad_variable_names_fail_before_bazel_runs() {
    let cli = cli("@dx_tools//:gitleaks", "DX-BIN");
    let error = run(cli).unwrap_err();
    assert!(
        error.to_string().contains("DX-BIN"),
        "the diagnostic must name the variable: {error}"
    );
}

#[test]
fn bad_labels_fail_before_bazel_runs() {
    let cli = cli("not a label", "DX_GITLEAKS_BIN");
    assert!(run(cli).is_err());
}

#[test]
fn missing_github_env_without_print_or_file_is_a_usage_error() {
    let mut cli = cli("@dx_tools//:gitleaks", "DX_GITLEAKS_BIN");
    cli.print = false;
    cli.env_file = None;
    std::env::remove_var("GITHUB_ENV");
    let error = run(cli).unwrap_err();
    match error {
        dx_tool_resolve::ResolveError::Usage { detail } => {
            assert!(detail.contains("GITHUB_ENV"), "{detail}");
        }
        other => panic!("a missing GITHUB_ENV must be a usage error: {other}"),
    }
}
