use std::path::Path;

use super::*;

struct FakeBazel {
    cquery_stdout: String,
    output_base: String,
}

impl Bazel for FakeBazel {
    fn cquery_files(&self, _label: &str, _args: &[String]) -> Result<String, ResolveError> {
        Ok(self.cquery_stdout.clone())
    }

    fn output_base(&self) -> Result<String, ResolveError> {
        Ok(self.output_base.clone())
    }
}

struct FailingBazel {
    detail: String,
}

impl Bazel for FailingBazel {
    fn cquery_files(&self, label: &str, _args: &[String]) -> Result<String, ResolveError> {
        Err(ResolveError::CqueryFailed {
            label: label.to_owned(),
            detail: self.detail.clone(),
        })
    }

    fn output_base(&self) -> Result<String, ResolveError> {
        Ok("/base".to_owned())
    }
}

fn fake_with(output_base: &str, cquery_stdout: &str) -> FakeBazel {
    FakeBazel {
        cquery_stdout: cquery_stdout.to_owned(),
        output_base: output_base.to_owned(),
    }
}

#[test]
fn context_dir_prefers_the_explicit_workspace() {
    let dir = context_dir(
        Some(Path::new("/explicit")),
        Some(std::ffi::OsStr::new("/invocation")),
        Path::new("/current"),
    );
    assert_eq!(dir, Path::new("/explicit"));
}

#[test]
fn context_dir_falls_back_to_the_invocation_root_then_cwd() {
    let dir = context_dir(
        None,
        Some(std::ffi::OsStr::new("/invocation")),
        Path::new("/current"),
    );
    assert_eq!(dir, Path::new("/invocation"));
    let dir = context_dir(None, None, Path::new("/current"));
    assert_eq!(dir, Path::new("/current"));
    let dir = context_dir(None, Some(std::ffi::OsStr::new("")), Path::new("/current"));
    assert_eq!(dir, Path::new("/current"));
}

#[test]
fn failure_summaries_stay_on_one_line() {
    assert_eq!(exit_detail(Some(1), b""), "exit 1");
    assert_eq!(exit_detail(None, b""), "exit -1");
    assert_eq!(
        exit_detail(Some(1), b"  \nsecond\nthird\n"),
        "exit 1: second"
    );
    assert_eq!(first_line(b""), None);
}

#[test]
fn var_names_accept_letters_digits_and_underscores() {
    for valid in ["DX_GITLEAKS_BIN", "_X", "A1", "a"] {
        validate_var(valid).unwrap_or_else(|_| panic!("{valid:?} must validate"));
    }
}

#[test]
fn var_names_reject_empty_dotted_and_dashed_names() {
    for invalid in ["", "DX-BIN", "DX.BIN", "1DX", "DX BIN", "DX=BIN"] {
        assert!(
            validate_var(invalid).is_err(),
            "{invalid:?} must not validate"
        );
    }
}

#[test]
fn labels_reject_empty_and_whitespace() {
    for invalid in ["", "  ", "@dx_tools//:git leaks", "label\nlabel"] {
        assert!(
            validate_label(invalid).is_err(),
            "{invalid:?} must not validate"
        );
    }
    validate_label("@dx_tools//:gitleaks").expect("@dx_tools//:gitleaks must validate");
}

#[test]
fn selection_takes_the_last_non_empty_line() {
    let selected = select_artifact(
        "external/a/tool\n\nexternal/b/tool\n   \n",
        "@dx_tools//:gitleaks",
    )
    .expect("one artifact must be selected");
    assert_eq!(selected, "external/b/tool");
}

#[test]
fn selection_trims_each_line() {
    let selected = select_artifact("  external/a/tool  \r\n", "@dx_tools//:gitleaks")
        .expect("one artifact must be selected");
    assert_eq!(selected, "external/a/tool");
}

#[test]
fn selection_fails_closed_on_empty_output() {
    let error = select_artifact("  \n\n", "@dx_tools//:gitleaks").unwrap_err();
    assert!(
        error.to_string().contains("@dx_tools//:gitleaks"),
        "the diagnostic must name the label: {error}"
    );
}

#[test]
fn env_lines_render_name_equals_value() {
    let line = env_line("DX_GITLEAKS_BIN", Path::new("/base/external/g/tool"))
        .expect("the line must render");
    assert_eq!(line, "DX_GITLEAKS_BIN=/base/external/g/tool\n");
}

#[test]
fn env_lines_reject_bad_variable_names() {
    assert!(env_line("DX-BIN", Path::new("/base/tool")).is_err());
}

#[test]
fn resolve_joins_the_base_with_the_selected_artifact() {
    let dir = tempfile::tempdir().expect("scratch");
    let tool = dir.path().join("tool");
    std::fs::write(&tool, b"fake").expect("fixture");
    let rel = tool
        .strip_prefix(dir.path())
        .expect("prefix")
        .to_string_lossy()
        .into_owned();
    let bazel = fake_with(
        &dir.path().to_string_lossy(),
        &format!("external/a/tool\n{rel}\n"),
    );
    let resolved = resolve(&bazel, "@dx_tools//:gitleaks", &[]).expect("the artifact must resolve");
    assert_eq!(resolved, tool);
}

#[test]
fn resolve_reports_cquery_failures_with_the_label() {
    let bazel = FailingBazel {
        detail: "exit 7: no such target".to_owned(),
    };
    let error = resolve(&bazel, "@dx_tools//:gitleaks", &[]).unwrap_err();
    let text = error.to_string();
    assert!(text.contains("@dx_tools//:gitleaks"), "{text}");
    assert!(text.contains("no such target"), "{text}");
}

#[test]
fn resolve_refuses_a_missing_artifact_file() {
    let bazel = fake_with("/base", "external/gone/tool\n");
    let error = resolve(&bazel, "@dx_tools//:gitleaks", &[]).unwrap_err();
    match error {
        ResolveError::MissingArtifact { label, path } => {
            assert_eq!(label, "@dx_tools//:gitleaks");
            assert!(path.ends_with("external/gone/tool"), "{path}");
        }
        other => panic!("missing files must fail closed: {other}"),
    }
}

#[test]
fn resolve_rejects_bad_labels_before_touching_bazel() {
    struct PanicBazel;
    impl Bazel for PanicBazel {
        fn cquery_files(&self, _label: &str, _args: &[String]) -> Result<String, ResolveError> {
            panic!("cquery must not run for a bad label")
        }
        fn output_base(&self) -> Result<String, ResolveError> {
            panic!("output_base must not run for a bad label")
        }
    }
    assert!(resolve(&PanicBazel, "not a label", &[]).is_err());
}

#[test]
fn env_file_appends_keep_existing_entries() {
    let dir = tempfile::tempdir().expect("scratch");
    let env = dir.path().join("github_env");
    std::fs::write(&env, "FIRST=1\n").expect("fixture");
    append_env_file(&env, "DX_GITLEAKS_BIN=/base/tool\n").expect("append");
    let text = std::fs::read_to_string(&env).expect("read");
    assert_eq!(text, "FIRST=1\nDX_GITLEAKS_BIN=/base/tool\n");
}

#[test]
fn every_failure_diagnostic_carries_the_resolve_code() {
    let errors = [
        ResolveError::Usage {
            detail: "bad".to_owned(),
        },
        ResolveError::NoArtifact {
            label: "l".to_owned(),
        },
        ResolveError::OutputBaseFailed {
            detail: "d".to_owned(),
        },
        ResolveError::MissingArtifact {
            label: "l".to_owned(),
            path: "/p".to_owned(),
        },
        ResolveError::EnvWrite {
            var: "V".to_owned(),
            path: "/e".to_owned(),
            detail: "d".to_owned(),
        },
    ];
    for error in errors {
        assert!(
            error.to_string().starts_with("tool_resolve_failed: "),
            "{error}"
        );
    }
}
