use super::real_core::*;
use super::real_fixtures::*;
use super::*;
use crate::{full_selection, run_convergence, synthetic::per_path};
use quality_adapter::parsers;
use quality_result::encode_validated;
use quality_result::proto::Convergence;
use quality_result::MAX_COMPLETED_ROUNDS;

#[test]
fn escaping_paths_fail_the_action() {
    let backend = backend_for("rustfmt", rustfmt_tool(), roundtrip_rustfmt);
    let err = backend
        .diagnose("rustfmt", "format", &single("../evil.rs", "x\n"))
        .expect_err("escape fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    let err = backend
        .apply_fix("rustfmt", "../evil.rs", "x\n", "format")
        .expect_err("escape fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
}

#[test]
fn escaping_tool_files_fail_the_action() {
    let tool = RealTool {
        tool_files: vec![("../evil".to_owned(), b"".to_vec())],
        edition: Some("2021".to_owned()),
        ..plain_tool()
    };
    let backend = backend_for("rustfmt", tool, roundtrip_rustfmt);
    let err = backend
        .diagnose("rustfmt", "format", &single("src/main.rs", "x\n"))
        .expect_err("escape fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("materialize"));
}

#[test]
fn unusable_scratch_parent_fails_the_action() {
    let backend = RealBackend {
        tools: BTreeMap::from([("rustfmt".to_owned(), rustfmt_tool())]),
        scratch_parent: PathBuf::from("/nonexistent-dx-scratch-parent"),
        spawn: roundtrip_rustfmt,
        argv_limit: quality_adapter::commands::windows_argv_limit,
    };
    let err = backend
        .diagnose("rustfmt", "format", &single("src/main.rs", "x\n"))
        .expect_err("scratch fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("scratch"));
    let err = backend
        .apply_fix("rustfmt", "src/main.rs", "x\n", "format")
        .expect_err("scratch fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
}

#[test]
fn escaping_configs_fail_the_action() {
    let tool = RealTool {
        config_rel: Some("../evil.toml".to_owned()),
        ..plain_tool()
    };
    let backend = backend_for("taplo", tool, taplo_either);
    let err = backend
        .diagnose("taplo", "lint", &single("a.toml", "a = 1\n"))
        .expect_err("escape fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("scratch config"));
    let tool = RealTool {
        config_rel: Some("../evil.toml".to_owned()),
        edition: Some("2021".to_owned()),
        ..plain_tool()
    };
    let backend = backend_for("rustfmt", tool, rustfmt_hinted);
    let err = backend
        .diagnose("rustfmt", "format", &single("src/main.rs", "x\n"))
        .expect_err("escape fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
}

#[test]
fn unexpected_rustfmt_fix_failure_surfaces_status_and_stderr() {
    let backend = backend_for("rustfmt", rustfmt_tool(), failing_fix);
    let err = backend
        .apply_fix("rustfmt", "src/main.rs", "x  \n", "format")
        .expect_err("rustfmt failure surfaces");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(
        err.to_string()
            .ends_with("rustfmt failed with exit 1: syntax error"),
        "{err}"
    );
}

#[test]
fn fix_without_output_file_fails_the_action() {
    let backend = backend_for("rustfmt", rustfmt_tool(), deleting_fix);
    let err = backend
        .apply_fix("rustfmt", "src/main.rs", "x\n", "format")
        .expect_err("re-read fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("re-read"));
}

#[test]
fn non_utf8_fix_fails_the_action() {
    let backend = backend_for("rustfmt", rustfmt_tool(), binary_fix);
    let err = backend
        .apply_fix("rustfmt", "src/main.rs", "x\n", "format")
        .expect_err("encoding fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
}

fn standardrb_crash(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: b"1 error occurred:\nAn error occurred while Style/Foo cop was inspecting a.rb.\n"
            .to_vec(),
    })
}

#[test]
fn standardrb_fix_fails_on_a_cop_crash_despite_exit_one() {
    let backend = backend_for("standardrb", plain_tool(), standardrb_crash);
    let err = backend
        .apply_fix("standardrb", "a.rb", "x = 1\n", "lint")
        .expect_err("crash fails the fix");
    assert!(matches!(err, RunnerError::ToolExecution { .. }), "{err}");
    assert!(err.to_string().contains("cop crashed"), "{err}");
}

#[test]
fn error_display_reports_variants() {
    let rendered = format!(
        "{}",
        RunnerError::ToolOutput {
            tool_id: "taplo".to_owned(),
            detail: "bad grammar".to_owned(),
        }
    );
    assert!(rendered.contains("invalid tool output"));
}

fn buildifier_fix_ok(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    std::fs::write(last_file(argv), "fixed\n").expect("fix writes back");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn taplo_fix_ok(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    std::fs::write(last_file(argv), "a = 1\n").expect("fix writes back");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn check_ok_fix_missing(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    if argv.iter().any(|arg| arg == "--check") {
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "fix binary gone"))
}

fn check_ok_fix_poisons(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    if argv.iter().any(|arg| arg == "--check") {
        let bytes = std::fs::read(&file).expect("checked file is materialized");
        if bytes == b"POISON\n" {
            return Err(io::Error::new(io::ErrorKind::Other, "poisoned terminal"));
        }
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    std::fs::write(&file, b"POISON\n").expect("fix writes back");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
fn production_constructor_resolves_tools() {
    let tools = BTreeMap::from([("rustfmt".to_owned(), plain_tool())]);
    let backend = RealBackend::new(tools, std::env::temp_dir());
    assert!(backend.supports("rustfmt"));
    assert!(!backend.supports("nope"));
}

#[test]
fn buildifier_fix_rewrites_the_file() {
    let backend = backend_for("buildifier", plain_tool(), buildifier_fix_ok);
    let fixed = backend
        .apply_fix("buildifier", "a.bzl", "x = 1\n", "format")
        .expect("fixed");
    assert_eq!(fixed, "fixed\n");
}

#[test]
fn taplo_fix_rewrites_the_file() {
    let backend = backend_for("taplo", plain_tool(), taplo_fix_ok);
    let fixed = backend
        .apply_fix("taplo", "a.toml", "a=1\n", "lint")
        .expect("fixed");
    assert_eq!(fixed, "a = 1\n");
}

#[test]
fn keep_sorted_lint_reports_and_fix_sorts() {
    let backend = backend_for("keep_sorted", plain_tool(), keep_sorted_behavior);
    let findings = backend
        .diagnose(
            "keep_sorted",
            "lint",
            &single("notes.txt", KEEP_SORTED_DIRTY),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "keep_sorted");
    assert_eq!(findings[0].message, KEEP_SORTED_MESSAGE);
    assert_eq!(findings[0].path, "notes.txt");
    assert_eq!(
        (findings[0].start_byte, findings[0].end_byte),
        (Some(20), Some(20))
    );
    assert!(!findings[0].fixable);
    assert!(backend
        .diagnose(
            "keep_sorted",
            "lint",
            &single("notes.txt", KEEP_SORTED_CLEAN)
        )
        .expect("diagnosed")
        .is_empty());
    let fixed = backend
        .apply_fix("keep_sorted", "notes.txt", KEEP_SORTED_DIRTY, "lint")
        .expect("fixed");
    assert_eq!(fixed, KEEP_SORTED_CLEAN);
    assert!(backend
        .diagnose("keep_sorted", "lint", &single("notes.txt", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn keep_sorted_lint_pipeline_fixes_dirty_files_to_stable() {
    let backend = backend_for("keep_sorted", plain_tool(), keep_sorted_behavior);
    let stages = vec![stage("keep_sorted", &["text"], &["notes.txt"])];
    let files = vec![file("notes.txt", KEEP_SORTED_DIRTY)];
    let result = run_real_pipeline("//quality:test", "lint", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert_eq!(result.initial_diagnostics[0].tool_id, "keep_sorted");
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].path, "notes.txt");
    assert!(encode_validated(&result).is_ok());
}

fn keep_sorted_empty_exit(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn keep_sorted_garbage(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Ok(ChildOutput {
        code: Some(1),
        stdout: b"not json".to_vec(),
        stderr: Vec::new(),
    })
}

fn keep_sorted_elsewhere(
    _argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    Ok(ChildOutput {
        code: Some(1),
        stdout: b"[{\"path\":\"/elsewhere/notes.txt\",\"lines\":{\"start\":2,\"end\":4},\"message\":\"These lines are out of order.\"}]".to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn keep_sorted_delegated_fix_is_check_only() {
    let fixture = upstream_file("keep-sorted-fix", "notes.txt:4: block is not sorted\n");
    let path = fixture.path().to_path_buf();
    let backend = backend_for("keep_sorted", delegated_tool(path.clone()), no_spawn);
    let findings = backend
        .diagnose(
            "keep_sorted",
            "lint",
            &single("notes.txt", KEEP_SORTED_DIRTY),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    let patched = backend
        .apply_fix("keep_sorted", "notes.txt", KEEP_SORTED_DIRTY, "lint")
        .expect("check-only keeps input");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(patched, KEEP_SORTED_DIRTY);
}

#[test]
fn keep_sorted_direct_failures_fail_the_action() {
    for spawn in [
        keep_sorted_empty_exit,
        keep_sorted_garbage,
        keep_sorted_elsewhere,
    ] {
        let backend = backend_for("keep_sorted", plain_tool(), spawn);
        let err = backend
            .diagnose(
                "keep_sorted",
                "lint",
                &single("notes.txt", KEEP_SORTED_DIRTY),
            )
            .expect_err("direct failure fails");
        assert!(
            matches!(
                err,
                RunnerError::ToolOutput { .. } | RunnerError::UnplaceableFinding { .. }
            ),
            "unexpected error: {err}"
        );
    }
}

#[test]
fn ruff_lint_reports_and_fix_rereads_on_exit_one() {
    let backend = backend_for("ruff", plain_tool(), roundtrip_ruff);
    let findings = backend
        .diagnose("ruff", "lint", &single("a.py", "import os\n# UNFIXABLE\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "ruff");
    assert_eq!(findings[0].rule_id, "F401");
    assert_eq!(
        findings[0].severity,
        quality_result::proto::Severity::Error as i32
    );
    let fixed = backend
        .apply_fix("ruff", "a.py", "import os\n# UNFIXABLE\n", "lint")
        .expect("fixed");
    assert_eq!(fixed, "# UNFIXABLE\n");
    assert!(backend
        .diagnose("ruff", "lint", &single("a.py", "x = 1\n"))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn ruff_format_reports_and_rewrites() {
    let backend = backend_for("ruff", plain_tool(), roundtrip_ruff);
    let findings = backend
        .diagnose("ruff", "format", &single("a.py", "x = 1  \n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "unformatted");
    let fixed = backend
        .apply_fix("ruff", "a.py", "x = 1  \n", "format")
        .expect("fixed");
    assert_eq!(fixed, "x = 1\n");
}

#[test]
fn ruff_fix_follows_the_running_capability() {
    let backend = backend_for("ruff", plain_tool(), roundtrip_ruff);
    let fixed = backend
        .apply_fix("ruff", "a.py", "import os\nx = 1  \n", "lint")
        .expect("fixed");
    assert_eq!(fixed, "x = 1  \n");
    let fixed = backend
        .apply_fix("ruff", "a.py", "import os\nx = 1  \n", "format")
        .expect("fixed");
    assert_eq!(fixed, "import os\nx = 1\n");
}

#[test]
fn ruff_hinted_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("ruff.toml".to_owned()),
        tool_files: vec![("ruff.toml".to_owned(), b"".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("ruff", tool, roundtrip_ruff_hinted);
    let findings = backend
        .diagnose("ruff", "lint", &single("a.py", "import os\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "F401");
}

fn ty_search_path_argv(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert!(
        argv.windows(2)
            .any(|pair| pair[0] == "--extra-search-path" && pair[1] == "/wheels/site-packages"),
        "tool search path reaches ty"
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
fn ty_receives_tool_search_paths() {
    let tool = RealTool {
        search_paths: vec![PathBuf::from("/wheels/site-packages")],
        ..plain_tool()
    };
    let backend = backend_for("ty", tool, ty_search_path_argv);
    backend
        .diagnose("ty", "typecheck", &single("a.py", "x: int = 1\n"))
        .expect("diagnosed");
}

#[test]
fn ty_reports_and_is_check_only() {
    let backend = backend_for("ty", plain_tool(), roundtrip_ty);
    let findings = backend
        .diagnose("ty", "typecheck", &single("a.py", "x: int = BADTYPE\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "ty");
    assert_eq!(findings[0].rule_id, "invalid-assignment");
    assert_eq!(
        findings[0].severity,
        quality_result::proto::Severity::Error as i32
    );
    assert!(backend
        .diagnose("ty", "typecheck", &single("a.py", "x: int = 1\n"))
        .expect("diagnosed")
        .is_empty());
    let text = "x: int = BADTYPE\n";
    assert_eq!(
        backend
            .apply_fix("ty", "a.py", text, "typecheck")
            .expect("check-only"),
        text
    );
}

#[test]
fn pydoclint_reports_and_is_check_only() {
    let backend = backend_for("pydoclint", plain_tool(), roundtrip_pydoclint);
    let findings = backend
        .diagnose(
            "pydoclint",
            "lint",
            &single("a.py", "def foo():\n    NODOC\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "pydoclint");
    assert_eq!(findings[0].rule_id, "DOC201");
    assert_eq!(findings[0].path, "a.py");
    assert!(backend
        .diagnose(
            "pydoclint",
            "lint",
            &single("a.py", "\"\"\"Module.\"\"\"\n")
        )
        .expect("diagnosed")
        .is_empty());
    let text = "def foo():\n    NODOC\n";
    assert_eq!(
        backend
            .apply_fix("pydoclint", "a.py", text, "lint")
            .expect("check-only"),
        text
    );
}

#[test]
fn flake8_reports_and_is_check_only() {
    let backend = backend_for("flake8", plain_tool(), roundtrip_flake8);
    let findings = backend
        .diagnose("flake8", "lint", &single("a.py", "import os\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "flake8");
    assert_eq!(findings[0].rule_id, "F401");
    assert_eq!(findings[0].path, "a.py");
    assert_eq!(
        findings[0].severity,
        quality_result::proto::Severity::Error as i32
    );
    assert!(backend
        .diagnose("flake8", "lint", &single("a.py", "\"\"\"Module.\"\"\"\n"))
        .expect("diagnosed")
        .is_empty());
    let text = "import os\n";
    assert_eq!(
        backend
            .apply_fix("flake8", "a.py", text, "lint")
            .expect("check-only"),
        text
    );
}

#[test]
fn pylint_reports_and_is_check_only() {
    let backend = backend_for("pylint", plain_tool(), roundtrip_pylint);
    let findings = backend
        .diagnose("pylint", "lint", &single("a.py", "import os\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "pylint");
    assert_eq!(findings[0].rule_id, "W0611");
    assert_eq!(findings[0].path, "a.py");
    assert_eq!(
        findings[0].severity,
        quality_result::proto::Severity::Warning as i32
    );
    assert_eq!(
        (findings[0].start_byte, findings[0].end_byte),
        (Some(0), Some(9))
    );
    assert!(backend
        .diagnose("pylint", "lint", &single("a.py", "\"\"\"Module.\"\"\"\n"))
        .expect("diagnosed")
        .is_empty());
    let text = "import os\n";
    assert_eq!(
        backend
            .apply_fix("pylint", "a.py", text, "lint")
            .expect("check-only"),
        text
    );
}

#[test]
fn biome_lint_uses_pinned_defaults_and_is_check_only() {
    let backend = backend_for("biome", plain_tool(), biome_defaults);
    let findings = backend
        .diagnose(
            "biome",
            "lint",
            &single("src/a.js", "const unusedVar = 1;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "biome");
    assert_eq!(findings[0].rule_id, "lint/correctness/noUnusedVariables");
    assert_eq!(findings[0].path, "src/a.js");
    assert!(backend
        .diagnose("biome", "lint", &single("src/a.js", "const x = 1;\n"))
        .expect("diagnosed")
        .is_empty());
    let text = "const unusedVar = 1;\n";
    assert_eq!(
        backend
            .apply_fix("biome", "src/a.js", text, "lint")
            .expect("check-only"),
        text
    );
}

#[test]
fn biome_hinted_config_wins_over_defaults() {
    let tool = RealTool {
        config_rel: Some("cfg/biome.json".to_owned()),
        tool_files: vec![(
            "cfg/biome.json".to_owned(),
            b"{\"linter\":{\"enabled\":false}}".to_vec(),
        )],
        ..plain_tool()
    };
    let backend = backend_for("biome", tool, biome_hinted);
    let findings = backend
        .diagnose(
            "biome",
            "lint",
            &single("src/a.js", "const unusedVar = 1;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "lint/correctness/noUnusedVariables");
}

#[test]
fn biome_format_reports_and_fix_rewrites() {
    let backend = backend_for("biome", plain_tool(), roundtrip_biome);
    let findings = backend
        .diagnose(
            "biome",
            "format",
            &single("src/a.js", "const x = BADFMT;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "biome");
    assert_eq!(findings[0].rule_id, "");
    assert_eq!(findings[0].path, "src/a.js");
    assert!(backend
        .diagnose("biome", "format", &single("src/a.js", "const x = 1;\n"))
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("biome", "src/a.js", "const x = BADFMT;\n", "format")
            .expect("fixed"),
        "const x = 1;\n"
    );
}

fn eslint_tool() -> RealTool {
    RealTool {
        config_rel: Some("eslint.config.mjs".to_owned()),
        tool_files: vec![(
            "eslint.config.mjs".to_owned(),
            b"export default [];".to_vec(),
        )],
        ..plain_tool()
    }
}

#[test]
fn eslint_reports_and_fix_rereads_on_exit_1() {
    let backend = backend_for("eslint", eslint_tool(), roundtrip_eslint);
    let findings = backend
        .diagnose(
            "eslint",
            "lint",
            &single("src/a.js", "const unusedVar = 1;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "eslint");
    assert_eq!(findings[0].rule_id, "no-unused-vars");
    assert_eq!(findings[0].path, "src/a.js");
    assert!(backend
        .diagnose("eslint", "lint", &single("src/a.js", "const x = 1;\n"))
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("eslint", "src/a.js", "const unusedVar = 1;\n", "lint")
            .expect("fixed"),
        "const usedVar = 1;\n"
    );
}

#[test]
fn eslint_without_config_fails_the_action() {
    let backend = backend_for("eslint", plain_tool(), roundtrip_eslint);
    let err = backend
        .diagnose("eslint", "lint", &single("src/a.js", "const x = 1;\n"))
        .expect_err("missing config fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("eslint requires a config"));
    let err = backend
        .apply_fix("eslint", "src/a.js", "const x = 1;\n", "lint")
        .expect_err("missing config fails");
    assert!(err.to_string().contains("eslint requires a config"));
}

fn graphql_eslint_tool() -> RealTool {
    RealTool {
        config_rel: Some("api/graphql.json".to_owned()),
        tool_files: vec![(
            "api/graphql.json".to_owned(),
            br#"{"schema":"api/schema.graphql"}"#.to_vec(),
        )],
        ..plain_tool()
    }
}

#[test]
fn graphql_eslint_reports_findings_and_never_fixes() {
    let backend = backend_for(
        "graphql_eslint",
        graphql_eslint_tool(),
        roundtrip_graphql_eslint,
    );
    let findings = backend
        .diagnose(
            "graphql_eslint",
            "lint",
            &single("api/query.graphql", "{ user { email } }\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "graphql_eslint");
    assert_eq!(
        findings[0].rule_id,
        "@graphql-eslint/fields-on-correct-type"
    );
    assert_eq!(findings[0].path, "api/query.graphql");
    assert!(backend
        .diagnose(
            "graphql_eslint",
            "lint",
            &single("api/query.graphql", "{ user { id } }\n")
        )
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix(
                "graphql_eslint",
                "api/query.graphql",
                "{ user { email } }\n",
                "lint"
            )
            .expect("check-only"),
        "{ user { email } }\n"
    );
}

#[test]
fn graphql_eslint_without_config_fails_the_action() {
    let backend = backend_for("graphql_eslint", plain_tool(), roundtrip_graphql_eslint);
    let err = backend
        .diagnose(
            "graphql_eslint",
            "lint",
            &single("api/q.graphql", "{ a }\n"),
        )
        .expect_err("missing config fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("graphql_eslint requires a config"));
    let err = backend
        .apply_fix("graphql_eslint", "api/q.graphql", "{ a }\n", "lint")
        .expect_err("missing config fails");
    assert!(err.to_string().contains("graphql_eslint requires a config"));
}

#[test]
fn graphql_eslint_usage_errors_are_never_clean() {
    let backend = backend_for(
        "graphql_eslint",
        graphql_eslint_tool(),
        roundtrip_graphql_eslint,
    );
    let err = backend
        .diagnose(
            "graphql_eslint",
            "lint",
            &single("api/q.graphql", "BROKEN\n"),
        )
        .expect_err("exit 2 fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("exit 2"), "{err}");
    assert!(err.to_string().contains("matches no file"), "{err}");
}

fn recording_eslint(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let program = argv
        .first()
        .map(|arg| arg.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert!(
        Path::new(&program).is_absolute(),
        "the runner spells the binary absolutely"
    );
    let manifest = env
        .iter()
        .find(|(key, _)| key == "RUNFILES_MANIFEST_FILE")
        .map(|(_, value)| value.to_string_lossy().into_owned());
    assert_eq!(
        manifest.as_deref(),
        Some(format!("{program}.runfiles_manifest").as_str()),
        "the launch names the manifest beside its own binary"
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "JS_BINARY__NO_CD_BINDIR" && value == "1"),
        "declared tool env reaches the launch"
    );
    let file = last_file(argv);
    assert!(
        Path::new(&file).starts_with(cwd),
        "checked files live under the launch cwd"
    );
    roundtrip_eslint(argv, cwd, env)
}

#[test]
fn eslint_launch_keeps_argv_cwd_env_runfiles_and_diagnostics() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let binary = dir.path().join("eslint");
    std::fs::write(&binary, "launcher\n").expect("stage launcher");
    std::fs::write(
        dir.path().join("eslint.runfiles_manifest"),
        "_main/node/node /node\n",
    )
    .expect("stage manifest");
    let tool = RealTool {
        binary,
        extra_env: vec![("JS_BINARY__NO_CD_BINDIR".to_owned(), "1".to_owned())],
        ..eslint_tool()
    };
    let backend = backend_for("eslint", tool, recording_eslint);
    let findings = backend
        .diagnose(
            "eslint",
            "lint",
            &single("src/a.js", "const unusedVar = 1;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "eslint");
    assert_eq!(findings[0].rule_id, "no-unused-vars");
    assert_eq!(findings[0].path, "src/a.js");
}

#[test]
fn eslint_without_a_program_fails_before_any_spawn() {
    let tool = RealTool {
        binary: PathBuf::new(),
        ..eslint_tool()
    };
    let backend = backend_for("eslint", tool, missing_spawn);
    let err = backend
        .diagnose("eslint", "lint", &single("src/a.js", "const x = 1;\n"))
        .expect_err("no program fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("names no executable"));
}

fn spaced_path_eslint(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let file = last_file(argv);
    assert!(
        Path::new(&file).starts_with(cwd),
        "the spaced file stays under the launch cwd: {file}"
    );
    assert!(
        Path::new(&file).ends_with("src dir/my file.js"),
        "the launch keeps the spaced path as one argv entry: {file}"
    );
    roundtrip_eslint(argv, cwd, env)
}

#[test]
fn eslint_launch_carries_paths_with_spaces_end_to_end() {
    let backend = backend_for("eslint", eslint_tool(), spaced_path_eslint);
    let findings = backend
        .diagnose(
            "eslint",
            "lint",
            &single("src dir/my file.js", "const unusedVar = 1;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "eslint");
    assert_eq!(findings[0].rule_id, "no-unused-vars");
    assert_eq!(findings[0].path, "src dir/my file.js");
}

#[test]
fn prettier_reports_relative_and_fix_rewrites() {
    let backend = backend_for("prettier", plain_tool(), roundtrip_prettier);
    let findings = backend
        .diagnose(
            "prettier",
            "format",
            &single("src/a.js", "const x = BADFMT;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "prettier");
    assert_eq!(findings[0].rule_id, "");
    assert_eq!(findings[0].path, "src/a.js");
    assert!(backend
        .diagnose("prettier", "format", &single("src/a.js", "const x = 1;\n"))
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("prettier", "src/a.js", "const x = BADFMT;\n", "format")
            .expect("fixed"),
        "const x = 1;\n"
    );
}

#[test]
fn biome_root_level_config_resolves_to_scratch_root() {
    let tool = RealTool {
        config_rel: Some("biome.json".to_owned()),
        tool_files: vec![("biome.json".to_owned(), b"{}".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("biome", tool, roundtrip_biome);
    let findings = backend
        .diagnose(
            "biome",
            "lint",
            &single("src/a.js", "const unusedVar = 1;\n"),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "lint/correctness/noUnusedVariables");
}

fn dialect_prettier(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let flags = argv
        .iter()
        .filter(|arg| *arg == "--dx-sql-dialect=postgresql")
        .count();
    assert_eq!(flags, 1, "check and fix both carry the dialect once");
    roundtrip_prettier(argv, cwd, env)
}

#[test]
fn prettier_check_and_fix_carry_the_sql_dialect() {
    let tool = RealTool {
        dialect: Some("postgresql".to_owned()),
        ..plain_tool()
    };
    let backend = backend_for("prettier", tool, dialect_prettier);
    let text = "select BADFMT;\n";
    assert_eq!(
        backend
            .diagnose("prettier", "format", &single("src/a.sql", text))
            .expect("diagnosed")
            .len(),
        1
    );
    assert_eq!(
        backend
            .apply_fix("prettier", "src/a.sql", text, "format")
            .expect("fixed"),
        "select 1;\n"
    );
}

#[test]
fn prettier_apply_fix_is_format_only() {
    let backend = backend_for("prettier", plain_tool(), roundtrip_prettier);
    let text = "const x = BADFMT;\n";
    assert_eq!(
        backend
            .apply_fix("prettier", "src/a.js", text, "lint")
            .expect("non-format fix is a no-op"),
        text
    );
}

#[test]
fn biome_fix_failure_keeps_original_text() {
    let backend = backend_for("biome", plain_tool(), failing_fix);
    let text = "const x = BADFMT;\n";
    assert_eq!(
        backend
            .apply_fix("biome", "src/a.js", text, "format")
            .expect("failed fix keeps text"),
        text
    );
}

#[test]
fn prettier_fix_failure_keeps_original_text() {
    let backend = backend_for("prettier", plain_tool(), failing_fix);
    let text = "const x = BADFMT;\n";
    assert_eq!(
        backend
            .apply_fix("prettier", "src/a.js", text, "format")
            .expect("failed fix keeps text"),
        text
    );
}

fn prettier_config_argv(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    prettier_config_argv_ending(argv, cwd, env, "cfg/.prettierrc.json")
}

fn prettier_external_config_argv(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    prettier_config_argv_ending(
        argv,
        cwd,
        env,
        "_external/shared_prettier+/cfg/.prettierrc.json",
    )
}

fn never_spawned(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    panic!("a config that escapes the scratch must fail before the tool starts");
}

fn prettier_config_argv_ending(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
    suffix: &str,
) -> io::Result<ChildOutput> {
    let args: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let bound: Vec<&str> = args
        .iter()
        .filter_map(|arg| arg.strip_prefix("--dx-config="))
        .collect();
    assert_eq!(bound.len(), 1, "hinted config passes one --dx-config");
    for reserved in ["--config", "--no-config", "--ignore-path"] {
        assert!(
            !args
                .iter()
                .any(|arg| arg == reserved || arg.starts_with(&format!("{reserved}="))),
            "{reserved} is owned by the prettier shim"
        );
    }
    let cfg = Path::new(bound[0]);
    assert!(cfg.is_absolute());
    assert!(cfg.ends_with(suffix));
    assert!(cfg.exists());
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
fn prettier_hinted_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("cfg/.prettierrc.json".to_owned()),
        tool_files: vec![("cfg/.prettierrc.json".to_owned(), b"{}".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("prettier", tool, prettier_config_argv);
    let findings = backend
        .diagnose("prettier", "format", &single("src/a.js", "const x = 1;\n"))
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
fn prettier_external_config_reaches_the_tool() {
    let rel = "_external/shared_prettier+/cfg/.prettierrc.json";
    let tool = RealTool {
        config_rel: Some(rel.to_owned()),
        tool_files: vec![(rel.to_owned(), b"{}".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("prettier", tool, prettier_external_config_argv);
    let findings = backend
        .diagnose("prettier", "format", &single("src/a.js", "const x = 1;\n"))
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
fn prettier_raw_parent_dir_config_never_reaches_the_tool() {
    let rel = "../shared_prettier+/cfg/.prettierrc.json";
    let make = || RealTool {
        config_rel: Some(rel.to_owned()),
        tool_files: vec![(rel.to_owned(), b"{}".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("prettier", make(), never_spawned);
    let err = backend
        .diagnose("prettier", "format", &single("src/a.js", "const x = 1;\n"))
        .expect_err("escaping config must fail");
    assert!(format!("{err:?}").contains("escapes scratch"), "{err:?}");
    let backend = backend_for("prettier", make(), never_spawned);
    let err = backend
        .apply_fix("prettier", "src/a.js", "const x = 1;\n", "format")
        .expect_err("escaping config must fail in fix too");
    assert!(format!("{err:?}").contains("escapes scratch"), "{err:?}");
}

#[test]
fn eslint_fix_failure_keeps_original_text() {
    let backend = backend_for("eslint", eslint_tool(), fatal_fix);
    let text = "const unusedVar = 1;\n";
    assert_eq!(
        backend
            .apply_fix("eslint", "src/a.js", text, "lint")
            .expect("failed fix keeps text"),
        text
    );
}

#[test]
fn biome_prettier_identical_output_converges_stable() {
    let stages = vec![
        stage("biome", &["json"], &["config/data.json"]),
        stage("prettier", &["json"], &["config/data.json"]),
    ];
    let mut initial = BTreeMap::new();
    initial.insert("config/data.json".to_owned(), "{\"a\":1}\n".to_owned());
    let agree = |_: &str, _: &str, _: &str| Ok("{\n  \"a\": 1\n}\n".to_owned());
    let (terminal, completed, convergence) = run_convergence(
        &initial,
        &stages,
        MAX_COMPLETED_ROUNDS,
        &full_selection(&stages),
        per_path(agree),
    )
    .expect("converged");
    assert_eq!(convergence, Convergence::Stable);
    assert_eq!(completed, 2);
    assert_eq!(terminal["config/data.json"], "{\n  \"a\": 1\n}\n");
}

#[test]
fn biome_prettier_conflicting_output_reports_oscillation() {
    let stages = vec![
        stage("biome", &["javascript"], &["src/app.js"]),
        stage("prettier", &["javascript"], &["src/app.js"]),
    ];
    let mut initial = BTreeMap::new();
    initial.insert("src/app.js".to_owned(), "compact\n".to_owned());
    let normalize = |tool: &str, _: &str, text: &str| {
        if tool == "biome" {
            if text == "tabs\n" {
                Ok(text.to_owned())
            } else {
                Ok("tabs\n".to_owned())
            }
        } else if text == "spaces\n" {
            Ok(text.to_owned())
        } else {
            Ok("spaces\n".to_owned())
        }
    };
    let (terminal, completed, convergence) = run_convergence(
        &initial,
        &stages,
        MAX_COMPLETED_ROUNDS,
        &full_selection(&stages),
        per_path(normalize),
    )
    .expect("converged");
    assert_eq!(convergence, Convergence::Oscillation);
    assert_eq!(completed, 2);
    assert_eq!(terminal["src/app.js"], "spaces\n");
    assert_eq!(
        normalize("biome", "javascript", "tabs\n").expect("idempotent"),
        "tabs\n"
    );
    assert_eq!(
        normalize("prettier", "javascript", "spaces\n").expect("idempotent"),
        "spaces\n"
    );
}

#[test]
fn biome_prettier_reverse_order_reports_oscillation() {
    let stages = vec![
        stage("prettier", &["javascript"], &["src/app.js"]),
        stage("biome", &["javascript"], &["src/app.js"]),
    ];
    let mut initial = BTreeMap::new();
    initial.insert("src/app.js".to_owned(), "compact\n".to_owned());
    let normalize = |tool: &str, _: &str, text: &str| {
        if tool == "biome" {
            if text == "tabs\n" {
                Ok(text.to_owned())
            } else {
                Ok("tabs\n".to_owned())
            }
        } else if text == "spaces\n" {
            Ok(text.to_owned())
        } else {
            Ok("spaces\n".to_owned())
        }
    };
    let (terminal, completed, convergence) = run_convergence(
        &initial,
        &stages,
        MAX_COMPLETED_ROUNDS,
        &full_selection(&stages),
        per_path(normalize),
    )
    .expect("converged");
    assert_eq!(convergence, Convergence::Oscillation);
    assert_eq!(completed, 2);
    assert_eq!(terminal["src/app.js"], "tabs\n");
    assert_eq!(
        normalize("biome", "javascript", "tabs\n").expect("idempotent"),
        "tabs\n"
    );
    assert_eq!(
        normalize("prettier", "javascript", "spaces\n").expect("idempotent"),
        "spaces\n"
    );
}

#[test]
fn fix_failure_aborts_real_convergence() {
    let backend = backend_for("rustfmt", rustfmt_tool(), check_ok_fix_missing);
    let stages = vec![stage("rustfmt", &["rust"], &["src/main.rs"])];
    let files = vec![file("src/main.rs", "x  \n")];
    let err = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect_err("fix fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("spawn"));
}

#[test]
fn check_spawn_failure_aborts_initial_diagnose() {
    let backend = backend_for("rustfmt", rustfmt_tool(), missing_spawn);
    let stages = vec![stage("rustfmt", &["rust"], &["src/main.rs"])];
    let files = vec![file("src/main.rs", "x\n")];
    let err = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect_err("check spawn fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("spawn"));
}

#[test]
fn terminal_check_failure_aborts_the_pipeline() {
    let backend = backend_for("rustfmt", rustfmt_tool(), check_ok_fix_poisons);
    let stages = vec![stage("rustfmt", &["rust"], &["src/main.rs"])];
    let files = vec![file("src/main.rs", "x\n")];
    let err = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect_err("terminal check fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("spawn"));
}

fn ruff_fix_crashes(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    if argv.iter().any(|arg| arg == "--fix") {
        assert_ruff_hermetic(argv, env);
        return Ok(ChildOutput {
            code: Some(2),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    roundtrip_ruff(argv, cwd, env)
}

fn ty_garbage(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        argv.iter().any(|arg| arg == "--no-respect-ignore-files"),
        "ty never observes VCS state"
    );
    Ok(ChildOutput {
        code: Some(1),
        stdout: b"garbage\n".to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn ruff_format_clean_and_unterminated_fix() {
    let backend = backend_for("ruff", plain_tool(), roundtrip_ruff);
    assert!(backend
        .diagnose("ruff", "format", &single("a.py", "x = 1\n"))
        .expect("diagnosed")
        .is_empty());
    let fixed = backend
        .apply_fix("ruff", "a.py", "x = 1  ", "format")
        .expect("fixed");
    assert_eq!(fixed, "x = 1");
}

#[test]
fn ruff_fix_failure_keeps_input() {
    let backend = backend_for("ruff", plain_tool(), ruff_fix_crashes);
    let text = "import os\n";
    assert_eq!(
        backend
            .apply_fix("ruff", "a.py", text, "lint")
            .expect("kept"),
        text
    );
    assert_eq!(
        backend
            .apply_fix("ruff", "a.py", text, "format")
            .expect("kept"),
        text
    );
}

#[test]
fn ty_output_failure_aborts_diagnose() {
    let backend = backend_for("ty", plain_tool(), ty_garbage);
    let err = backend
        .diagnose("ty", "typecheck", &single("a.py", "x: int = 1\n"))
        .expect_err("parse fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
}

#[test]
fn reanchor_reports_unstaged_file_without_panicking() {
    let pairs = vec![("a.py".to_owned(), PathBuf::from("/scratch/a.py"))];
    let err = reanchor("ty", &pairs, "b.py").expect_err("unknown path fails");
    assert!(matches!(err, RunnerError::UnplaceableFinding { .. }));
}

/// The fixtures that report a path inside JSON must escape it, because a Windows path holds
/// backslashes and a bare one either fails the parse or decodes as an escape such as `\b`.
#[test]
fn json_reporting_fixtures_escape_a_path_that_names_a_backslash() {
    let dir = tempfile::Builder::new()
        .prefix("dx-backslash-")
        .tempdir_in(std::env::temp_dir())
        .expect("claim backslash fixture");
    let file = dir.path().join("a\\b.py");
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).expect("claim backslash fixture parent");
    }
    std::fs::write(&file, "import os\nlet unusedVar = 1;\n").expect("write backslash fixture");
    let absolute = file.to_string_lossy().into_owned();
    let relative = "a\\b.py";
    let ambient: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    let env = quality_adapter::exec::hermetic_env(dir.path(), &[], &ambient);
    let path = file.as_os_str().to_owned();
    let buildifier_argv = [OsString::from("buildifier"), path.clone()];
    let pylint_argv = [
        OsString::from("pylint"),
        OsString::from("--persistent=n"),
        OsString::from("--reports=n"),
        OsString::from("--score=n"),
        OsString::from("--output-format=json"),
        path.clone(),
    ];
    let eslint_argv = [
        OsString::from("eslint"),
        OsString::from("-f"),
        OsString::from("json"),
        OsString::from("-c"),
        path.clone(),
        path.clone(),
    ];
    let ruff_argv = [OsString::from("ruff"), path];

    let pylint = roundtrip_pylint(&pylint_argv, dir.path(), &env).expect("pylint fixture runs");
    let eslint = roundtrip_eslint(&eslint_argv, dir.path(), &env).expect("eslint fixture runs");
    let buildifier =
        buildifier_plain(&buildifier_argv, dir.path(), &env).expect("buildifier fixture runs");
    let ruff = ruff_behavior(&ruff_argv).expect("ruff fixture runs");

    let reports = [
        (
            "pylint",
            parsers::parse_pylint(&pylint.stdout, Some(4), &[relative]),
            relative,
        ),
        (
            "eslint",
            parsers::parse_eslint(&eslint.stdout, Some(1), &[absolute.as_str()]),
            absolute.as_str(),
        ),
        (
            "buildifier",
            parsers::parse_buildifier(&buildifier.stdout, &[], &[absolute.as_str()]),
            absolute.as_str(),
        ),
        (
            "ruff",
            parsers::parse_ruff(&ruff.stdout, Some(1), &[absolute.as_str()], &[]),
            absolute.as_str(),
        ),
    ];
    for (tool, parsed, want) in reports {
        let findings =
            parsed.unwrap_or_else(|err| panic!("{tool} fixture wrote unparseable JSON: {err}"));
        assert_eq!(findings[0].file, want, "{tool} lost the path");
    }
}

fn oversized_selection(count: usize) -> BTreeMap<String, String> {
    padded_selection(count, 96)
}

fn padded_selection(count: usize, pad: usize) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for index in 0..count {
        let name = format!("notes-{index:04}-{}.txt", "n".repeat(pad));
        let body = if index % 2 == 0 {
            KEEP_SORTED_DIRTY
        } else {
            KEEP_SORTED_CLEAN
        };
        files.insert(name, body.to_owned());
    }
    files
}

fn batched_keep_sorted_spawns() -> usize {
    BATCHED_SPAWNS.load(std::sync::atomic::Ordering::SeqCst)
}

static BATCHED_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn batched_keep_sorted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    BATCHED_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_within_budget(argv);
    keep_sorted_multi_behavior(argv, cwd, env)
}

fn assert_within_budget(argv: &[OsString]) {
    let budget = quality_adapter::commands::argv_budget(&argv[0]);
    let (bytes, units) = quality_adapter::commands::argv_cost(argv);
    assert!(bytes <= budget, "spawn carries {bytes} argv bytes");
    assert!(units <= budget, "spawn carries {units} argv units");
}

fn python_selection(count: usize, pad: usize) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for index in 0..count {
        let name = format!("pkg/mod_{index:04}_{}.py", "p".repeat(pad));
        let body = if index % 2 == 0 {
            "import os \n"
        } else {
            "\"\"\"Module.\"\"\"\n"
        };
        files.insert(name, body.to_owned());
    }
    files
}

static RUFF_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn batched_ruff(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    RUFF_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_within_budget(argv);
    ruff_multi_behavior(argv, cwd, env)
}

static RUFF_EQUAL_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn batched_ruff_equal(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    RUFF_EQUAL_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_within_budget(argv);
    ruff_multi_behavior(argv, cwd, env)
}

static FLAKE8_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn batched_flake8(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    FLAKE8_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_within_budget(argv);
    flake8_multi_behavior(argv, cwd, env)
}

static LAUNCHER_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn batched_launcher(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    LAUNCHER_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        quality_adapter::commands::argv_budget(&argv[0]),
        quality_adapter::commands::LAUNCHER_BATCH_BUDGET
    );
    assert_within_budget(argv);
    ruff_multi_behavior(argv, cwd, env)
}

static TY_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn counting_ty(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    TY_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    roundtrip_ty(argv, cwd, env)
}

#[test]
fn ruff_large_selection_runs_batched() {
    let files = python_selection(300, 96);
    for (capability, kind) in [("lint", "F401"), ("format", "unformatted")] {
        RUFF_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
        let backend = backend_for("ruff", plain_tool(), batched_ruff);
        let first = backend
            .diagnose("ruff", capability, &files)
            .expect("batched ruff runs");
        let spawns = RUFF_SPAWNS.load(std::sync::atomic::Ordering::SeqCst);
        assert!(spawns > 1, "{capability} splits into {spawns} spawns");
        assert_eq!(first.len(), 150, "every other file is dirty");
        assert!(
            first.iter().all(|found| found.rule_id.contains(kind)),
            "{capability}"
        );
        let paths: Vec<&str> = first.iter().map(|found| found.path.as_str()).collect();
        let mut ordered = paths.clone();
        ordered.sort();
        assert_eq!(paths, ordered, "merged findings stay ordered");
        let second = backend
            .diagnose("ruff", capability, &files)
            .expect("batched ruff reruns");
        assert_eq!(first, second, "rerun merges identically");
    }
}

#[test]
fn ruff_batches_match_single_file_results() {
    let files = python_selection(120, 200);
    RUFF_EQUAL_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let batched = backend_for("ruff", plain_tool(), batched_ruff_equal);
    let merged = batched
        .diagnose("ruff", "lint", &files)
        .expect("batched ruff runs");
    assert!(
        RUFF_EQUAL_SPAWNS.load(std::sync::atomic::Ordering::SeqCst) > 1,
        "equivalence input spans batches"
    );
    let mut singles = Vec::new();
    for (path, body) in &files {
        let alone = backend_for("ruff", plain_tool(), roundtrip_ruff);
        singles.extend(
            alone
                .diagnose("ruff", "lint", &single(path, body))
                .expect("single runs"),
        );
    }
    singles.sort_by(|left, right| left.path.cmp(&right.path));
    assert_eq!(merged, singles);
    assert_eq!(merged.len(), 60);
}

#[test]
fn flake8_large_selection_runs_batched() {
    FLAKE8_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let files = python_selection(300, 96);
    let backend = backend_for("flake8", plain_tool(), batched_flake8);
    let findings = backend
        .diagnose("flake8", "lint", &files)
        .expect("batched flake8 runs");
    assert!(FLAKE8_SPAWNS.load(std::sync::atomic::Ordering::SeqCst) > 1);
    assert_eq!(findings.len(), 150);
    let paths: Vec<&str> = findings.iter().map(|found| found.path.as_str()).collect();
    let mut ordered = paths.clone();
    ordered.sort();
    assert_eq!(paths, ordered, "merged findings stay ordered");
}

#[test]
fn ty_over_budget_selection_fails_before_spawn() {
    TY_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let files = python_selection(300, 120);
    let backend = backend_for("ty", plain_tool(), counting_ty);
    let err = backend
        .diagnose("ty", "lint", &files)
        .expect_err("oversized ty selection fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(
        err.to_string()
            .contains("selection too large for one ty spawn"),
        "{err}"
    );
    assert_eq!(TY_SPAWNS.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn batch_launcher_splits_at_launcher_budget() {
    LAUNCHER_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let files = python_selection(120, 96);
    let tool = RealTool {
        binary: PathBuf::from("/fake/bin/ruff.bat"),
        ..plain_tool()
    };
    let backend = backend_for("ruff", tool, batched_launcher);
    let findings = backend
        .diagnose("ruff", "lint", &files)
        .expect("launcher batches stay under the guard");
    assert!(LAUNCHER_SPAWNS.load(std::sync::atomic::Ordering::SeqCst) > 2);
    assert_eq!(findings.len(), 60);
}

static FLAKY_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn flaky_keep_sorted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let at = FLAKY_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if at == 1 {
        return Err(io::Error::new(io::ErrorKind::Other, "terminal gone"));
    }
    keep_sorted_multi_behavior(argv, cwd, env)
}

static POISONED_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn poisoned_keep_sorted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let at = POISONED_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if at == 1 {
        return Ok(ChildOutput {
            code: Some(1),
            stdout: b"{<error".to_vec(),
            stderr: Vec::new(),
        });
    }
    keep_sorted_multi_behavior(argv, cwd, env)
}

#[test]
fn keep_sorted_large_selection_runs_batched_with_a_stable_merge() {
    BATCHED_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let files = oversized_selection(300);
    let backend = backend_for("keep_sorted", plain_tool(), batched_keep_sorted);
    let first = backend
        .diagnose("keep_sorted", "lint", &files)
        .expect("batched check runs");
    let spawns = batched_keep_sorted_spawns();
    assert!(
        spawns > 1,
        "oversized selection splits into {spawns} spawns"
    );
    assert_eq!(first.len(), 150, "every other file is dirty");
    let paths: Vec<&str> = first.iter().map(|found| found.path.as_str()).collect();
    let mut ordered = paths.clone();
    ordered.sort();
    assert_eq!(paths, ordered, "merged findings stay ordered");
    let second = backend
        .diagnose("keep_sorted", "lint", &files)
        .expect("batched check reruns");
    assert_eq!(first, second, "rerun merges identically");
}

#[test]
fn keep_sorted_batches_match_single_file_results() {
    let files = padded_selection(120, 200);
    BATCHED_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let batched = backend_for("keep_sorted", plain_tool(), batched_keep_sorted);
    let merged = batched
        .diagnose("keep_sorted", "lint", &files)
        .expect("batched check runs");
    assert!(
        batched_keep_sorted_spawns() > 1,
        "equivalence input spans batches"
    );
    let mut singles = Vec::new();
    for (path, body) in &files {
        let alone = backend_for("keep_sorted", plain_tool(), keep_sorted_multi_behavior);
        let mut one = BTreeMap::new();
        one.insert(path.clone(), body.clone());
        singles.extend(
            alone
                .diagnose("keep_sorted", "lint", &one)
                .expect("single runs"),
        );
    }
    let mut merged_paths: Vec<&str> = merged.iter().map(|found| found.path.as_str()).collect();
    let mut single_paths: Vec<&str> = singles.iter().map(|found| found.path.as_str()).collect();
    merged_paths.sort();
    single_paths.sort();
    assert_eq!(merged_paths, single_paths);
    assert_eq!(merged.len(), 60);
}

#[test]
fn keep_sorted_middle_batch_spawn_failure_aborts() {
    FLAKY_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let files = oversized_selection(300);
    let backend = backend_for("keep_sorted", plain_tool(), flaky_keep_sorted);
    let err = backend
        .diagnose("keep_sorted", "lint", &files)
        .expect_err("middle batch fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("batch 2/"), "{err}");
}

#[test]
fn keep_sorted_middle_batch_garbage_aborts() {
    POISONED_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let files = oversized_selection(300);
    let backend = backend_for("keep_sorted", plain_tool(), poisoned_keep_sorted);
    let err = backend
        .diagnose("keep_sorted", "lint", &files)
        .expect_err("middle batch fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
    assert!(err.to_string().contains("batch 2/"), "{err}");
}

#[test]
#[cfg(unix)]
fn qmllint_settings_replace_read_only_copies_and_links() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("scratch");
    let config = dir.path().join("qmllint.ini");
    std::fs::write(&config, b"[General]\n").expect("config");
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o444)).expect("chmod");
    let source = dir.path().join("app").join("Main.qml");
    std::fs::create_dir_all(source.parent().expect("parent")).expect("mkdir");
    let sources = vec![("app/Main.qml".to_owned(), source.clone())];
    let target = dir.path().join("app").join(".qmllint.ini");
    for _ in 0..2 {
        staging::place_qmllint_settings("qmllint", &config, &sources).expect("place");
        let mode = std::fs::metadata(&target)
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o644);
    }
    let outside = dir.path().join("outside.ini");
    std::fs::write(&outside, b"keep\n").expect("outside");
    std::fs::remove_file(&target).expect("remove");
    std::os::unix::fs::symlink(&outside, &target).expect("plant link");
    staging::place_qmllint_settings("qmllint", &config, &sources).expect("place over link");
    assert_eq!(std::fs::read(&outside).expect("outside"), b"keep\n");
    assert!(!std::fs::symlink_metadata(&target)
        .expect("target")
        .file_type()
        .is_symlink());
    assert_eq!(std::fs::read(&target).expect("target"), b"[General]\n");
}

#[test]
fn qml_import_roots_come_from_declared_module_uris() {
    let dir = tempfile::tempdir().expect("scratch");
    let write = |rel: &str, text: &str| {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
        (rel.to_owned(), path)
    };
    let context = vec![
        write("mods/Foo/qmldir", "module Foo\nBar 1.0 Bar.qml\n"),
        write("mods/Foo/Bar.qml", "import QtQuick\nItem {}\n"),
        write("libs/Acme/Ui/qmldir", "# comment\nmodule Acme.Ui\n"),
        write("plain/qmldir", "Thing 1.0 Thing.qml\n"),
    ];
    let roots = staging::qml_import_roots("qmllint", &context).expect("roots");
    assert_eq!(
        roots,
        vec![dir.path().join("mods"), dir.path().join("libs")]
    );
    let unmatched = vec![
        write("app/qmldir", "module App\n"),
        write("elsewhere/qmldir", "module Foo.Bar\n"),
    ];
    let skipped = staging::qml_import_roots("qmllint", &unmatched).expect("roots");
    assert!(skipped.is_empty(), "{skipped:?}");
}

fn buf_broken_import(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let root = PathBuf::from(argv.last().expect("root"));
    let item = root.join("item").join("v1").join("item.proto");
    let record = format!(
        "{{\"path\":{},\"start_line\":5,\"start_column\":19,\"type\":\"COMPILE\",\"message\":\"syntax error: unexpected '{{'\"}}\n",
        serde_json::to_string(&item.display().to_string()).expect("path")
    );
    Ok(ChildOutput {
        code: Some(100),
        stdout: record.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
fn buf_lint_names_a_broken_import() {
    const ORDER: &str =
        "syntax = \"proto3\";\n\npackage order.v1;\n\nimport \"item/v1/item.proto\";\n";
    const BROKEN: &str = "syntax = \"proto3\";\n\npackage item.v1;\n\nmessage Item {\n";
    let backend = backend_for("buf", plain_tool(), buf_broken_import);
    let resolve = single("item/v1/item.proto", BROKEN);
    let err = backend
        .diagnose_with_resolve(
            "buf",
            "lint",
            &single("order/v1/order.proto", ORDER),
            &BTreeMap::new(),
            &resolve,
        )
        .expect_err("broken import fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }), "{err}");
    let text = err.to_string();
    assert!(!text.contains("invalid tool output"), "{text}");
    assert!(text.contains("item/v1/item.proto:5:19"), "{text}");
    assert!(!text.contains("unchecked file"), "{text}");
}

fn hadolint_config_arg(argv: &[OsString]) -> PathBuf {
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("hadolint always gets --config");
    PathBuf::from(&argv[at + 1])
}

fn hadolint_report(argv: &[OsString], env: &[(OsString, OsString)]) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    for key in ["XDG_CONFIG_HOME", "HADOLINT_IGNORE"] {
        assert!(
            env.iter()
                .all(|(known, _)| !known.to_string_lossy().eq_ignore_ascii_case(key)),
            "{key} never reaches hadolint"
        );
    }
    let file = last_file(argv);
    assert_eq!(argv[argv.len() - 2], OsString::from("--"));
    let dirty = std::fs::read_to_string(&file)
        .expect("read staged Dockerfile")
        .contains("| tar");
    let stdout = if dirty {
        serde_json::json!([{
            "code": "DL4006",
            "column": 1,
            "file": file,
            "level": "warning",
            "line": 2,
            "message": "Set the SHELL option -o pipefail before RUN with a pipe in it.",
        }])
        .to_string()
    } else {
        "[]".to_owned()
    };
    Ok(ChildOutput {
        code: Some(i32::from(dirty)),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

fn hadolint_defaults(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let config = hadolint_config_arg(argv);
    assert!(
        config.ends_with("dx-hadolint-default/.hadolint.yaml"),
        "unbound hadolint uses the scratch default, got {config:?}"
    );
    let staged = std::fs::read(&config).expect("default hadolint config is staged");
    assert_eq!(staged, b"ignored: []\n");
    hadolint_report(argv, env)
}

fn hadolint_bound(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let config = hadolint_config_arg(argv);
    assert!(config.ends_with("cfg/.hadolint.yaml"), "{config:?}");
    let staged = std::fs::read(&config).expect("bound hadolint config is staged");
    assert_eq!(staged, b"ignored:\n  - DL3008\n");
    hadolint_report(argv, env)
}

const HADOLINT_DIRTY: &str =
    "FROM ubuntu:24.04\nRUN curl -sSL https://example.com/a.tgz | tar xz\n";

#[test]
fn hadolint_unbound_config_uses_the_scratch_default_and_is_check_only() {
    let backend = backend_for("hadolint", plain_tool(), hadolint_defaults);
    let findings = backend
        .diagnose(
            "hadolint",
            "lint",
            &single("img/Dockerfile", HADOLINT_DIRTY),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "hadolint");
    assert_eq!(findings[0].rule_id, "DL4006");
    assert_eq!(findings[0].path, "img/Dockerfile");
    assert!(backend
        .diagnose(
            "hadolint",
            "lint",
            &single("img/Dockerfile", "FROM ubuntu:24.04\n")
        )
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("hadolint", "img/Dockerfile", HADOLINT_DIRTY, "lint")
            .expect("check-only"),
        HADOLINT_DIRTY
    );
}

#[test]
fn hadolint_bound_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("cfg/.hadolint.yaml".to_owned()),
        tool_files: vec![(
            "cfg/.hadolint.yaml".to_owned(),
            b"ignored:\n  - DL3008\n".to_vec(),
        )],
        ..plain_tool()
    };
    let backend = backend_for("hadolint", tool, hadolint_bound);
    let findings = backend
        .diagnose("hadolint", "lint", &single("Dockerfile", HADOLINT_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "DL4006");
}

fn checkmake_config_arg(argv: &[OsString]) -> PathBuf {
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("checkmake always gets --config");
    PathBuf::from(&argv[at + 1])
}

fn checkmake_report(
    argv: &[OsString],
    env: &[(OsString, OsString)],
    confirm_config: bool,
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    for key in ["HOME", "XDG_CONFIG_HOME"] {
        assert!(
            env.iter()
                .all(|(known, _)| !known.to_string_lossy().eq_ignore_ascii_case(key)),
            "{key} never reaches checkmake"
        );
    }
    assert_eq!(&argv[1..4], ["--debug", "--output", "json"]);
    let file = last_file(argv);
    assert_eq!(argv[argv.len() - 2], OsString::from("--"));
    let dirty = std::fs::read_to_string(&file)
        .expect("read staged Makefile")
        .contains("all:");
    let stdout = if dirty {
        serde_json::json!([{
            "rule": "minphony",
            "violation": "Required target \"all\" must be declared PHONY.",
            "file_name": file,
            "line_number": 1,
        }])
        .to_string()
    } else {
        String::new()
    };
    let stderr = if confirm_config {
        format!(
            "2026/10/28 00:00:00 Using configuration file: {:?}\n",
            checkmake_config_arg(argv)
        )
    } else {
        "2026/10/28 00:00:00 Unable to parse config file \"checkmake.ini\", running with defaults\n"
            .to_owned()
    };
    Ok(ChildOutput {
        code: Some(i32::from(dirty)),
        stdout: stdout.into_bytes(),
        stderr: stderr.into_bytes(),
    })
}

fn checkmake_defaults(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let config = checkmake_config_arg(argv);
    assert!(
        config.ends_with("dx-checkmake-default/checkmake.ini"),
        "unbound checkmake uses the scratch default, got {config:?}"
    );
    assert_eq!(
        std::fs::read(&config).expect("default config is staged"),
        b""
    );
    checkmake_report(argv, env, true)
}

fn checkmake_bound(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let config = checkmake_config_arg(argv);
    assert!(config.ends_with("cfg/checkmake.ini"), "{config:?}");
    assert_eq!(
        std::fs::read(&config).expect("bound config is staged"),
        b"[default]\n"
    );
    checkmake_report(argv, env, true)
}

fn checkmake_ignores_config(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    checkmake_report(argv, env, false)
}

const CHECKMAKE_DIRTY: &str = "all:\n\t@true\n";

#[test]
fn checkmake_unbound_config_uses_the_scratch_default_and_is_check_only() {
    let backend = backend_for("checkmake", plain_tool(), checkmake_defaults);
    let findings = backend
        .diagnose("checkmake", "lint", &single("Makefile", CHECKMAKE_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "checkmake");
    assert_eq!(findings[0].rule_id, "minphony");
    assert_eq!(findings[0].path, "Makefile");
    assert!(backend
        .diagnose("checkmake", "lint", &single("Makefile", "X = 1\n"))
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("checkmake", "Makefile", CHECKMAKE_DIRTY, "lint")
            .expect("check-only"),
        CHECKMAKE_DIRTY
    );
}

#[test]
fn checkmake_bound_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("cfg/checkmake.ini".to_owned()),
        tool_files: vec![("cfg/checkmake.ini".to_owned(), b"[default]\n".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("checkmake", tool, checkmake_bound);
    let findings = backend
        .diagnose("checkmake", "lint", &single("Makefile", CHECKMAKE_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
}

#[test]
fn checkmake_config_fallback_is_an_error_not_a_clean_run() {
    let backend = backend_for("checkmake", plain_tool(), checkmake_ignores_config);
    assert!(backend
        .diagnose("checkmake", "lint", &single("Makefile", "X = 1\n"))
        .is_err());
}

fn gersemi_config_arg(argv: &[OsString]) -> PathBuf {
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("gersemi always gets --config");
    PathBuf::from(&argv[at + 1])
}

/// Stands in for gersemi: a file holding `BAD` is unformatted, `-i` rewrites it to `GOOD`.
fn gersemi_run(
    argv: &[OsString],
    env: &[(OsString, OsString)],
    config_tail: &str,
    config_bytes: &[u8],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    for key in ["HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME"] {
        assert!(
            env.iter()
                .all(|(known, _)| !known.to_string_lossy().eq_ignore_ascii_case(key)),
            "{key} never reaches gersemi"
        );
    }
    let config = gersemi_config_arg(argv);
    assert!(config.ends_with(config_tail), "{config:?}");
    assert_eq!(
        std::fs::read(&config).expect("config is staged"),
        config_bytes
    );
    let flags: Vec<String> = argv[1..]
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    for flag in ["--no-cache", "--no-color", "-w", "1", "--"] {
        assert!(flags.iter().any(|arg| arg == flag), "{flag} is always set");
    }
    assert_eq!(argv[argv.len() - 2], OsString::from("--"));
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let dirty = text.contains("BAD");
    if flags.iter().any(|arg| arg == "-i") {
        assert!(!flags.iter().any(|arg| arg == "--check"));
        std::fs::write(&file, text.replace("BAD", "GOOD")).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    assert!(flags.iter().any(|arg| arg == "--check"));
    assert!(flags.iter().any(|arg| arg == "--diff"));
    if !dirty {
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(1),
        stdout: format!("--- {file}\n+++ {file}\n@@ -1 +1 @@\n-BAD\n+GOOD\n").into_bytes(),
        stderr: format!("{file} would be reformatted\n").into_bytes(),
    })
}

fn gersemi_defaults(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    gersemi_run(argv, env, "dx-gersemi-default/.gersemirc", b"")
}

fn gersemi_bound(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    gersemi_run(argv, env, "cfg/.gersemirc", b"indent: 2\n")
}

const GERSEMI_DIRTY: &str = "BAD\n";

#[test]
fn gersemi_unbound_config_uses_the_scratch_default_and_check_never_writes() {
    let backend = backend_for("gersemi", plain_tool(), gersemi_defaults);
    let findings = backend
        .diagnose(
            "gersemi",
            "format",
            &single("CMakeLists.txt", GERSEMI_DIRTY),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "gersemi");
    assert_eq!(findings[0].path, "CMakeLists.txt");
    assert!(backend
        .diagnose("gersemi", "format", &single("CMakeLists.txt", "GOOD\n"))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn gersemi_fix_rewrites_and_a_recheck_is_clean() {
    let backend = backend_for("gersemi", plain_tool(), gersemi_defaults);
    let fixed = backend
        .apply_fix("gersemi", "CMakeLists.txt", GERSEMI_DIRTY, "format")
        .expect("fixed");
    assert_eq!(fixed, "GOOD\n");
    assert!(backend
        .diagnose("gersemi", "format", &single("CMakeLists.txt", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn gersemi_bound_config_reaches_the_tool_on_check_and_fix() {
    let tool = RealTool {
        config_rel: Some("cfg/.gersemirc".to_owned()),
        tool_files: vec![("cfg/.gersemirc".to_owned(), b"indent: 2\n".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("gersemi", tool, gersemi_bound);
    let findings = backend
        .diagnose(
            "gersemi",
            "format",
            &single("CMakeLists.txt", GERSEMI_DIRTY),
        )
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        backend
            .apply_fix("gersemi", "CMakeLists.txt", GERSEMI_DIRTY, "format")
            .expect("fixed"),
        "GOOD\n"
    );
}

fn gersemi_exit_one_without_a_diff(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: b"[Errno 2] No such file or directory: '/x/.gersemirc'\n".to_vec(),
    })
}

#[test]
fn gersemi_failing_without_a_diff_is_an_error_not_a_clean_run() {
    let backend = backend_for("gersemi", plain_tool(), gersemi_exit_one_without_a_diff);
    assert!(backend
        .diagnose("gersemi", "format", &single("CMakeLists.txt", "GOOD\n"))
        .is_err());
}

fn elvis_report(argv: &[OsString], cwd: &Path, config_tail: &str) -> io::Result<ChildOutput> {
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("elvis always gets --config");
    let config = PathBuf::from(&argv[at + 1]);
    assert!(config.ends_with(config_tail), "{config:?}");
    assert!(config.is_file(), "the elvis config is staged: {config:?}");
    assert_eq!(
        argv[at + 2..at + 5],
        [
            OsString::from("--output-format"),
            OsString::from("parsable"),
            OsString::from("rock")
        ]
    );
    let file = PathBuf::from(&argv[argv.len() - 1]);
    assert!(
        file.is_relative(),
        "elvis globs match cwd-relative paths: {file:?}"
    );
    let text = std::fs::read_to_string(cwd.join(&file)).expect("read staged module");
    if !text.contains("io:format") {
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(1),
        stdout: format!(
            "{}:2:no_debug_call:At line 2, column 6, an unexpected debug call to 'io:format/1' was found.\n",
            file.display()
        )
        .into_bytes(),
        stderr: b"Elvis: linting failed\n".to_vec(),
    })
}

fn elvis_defaults(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("config");
    let staged = std::fs::read_to_string(&argv[at + 1]).expect("default elvis config");
    assert!(staged.contains("\"**/*.erl\""), "{staged}");
    elvis_report(argv, cwd, "dx-elvis-default/elvis.config")
}

fn elvis_bound(
    argv: &[OsString],
    cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    elvis_report(argv, cwd, "cfg/elvis.config")
}

const ELVIS_DIRTY: &str = "-module(a).\nf() -> io:format(\"x\").\n";

#[test]
fn elvis_unbound_config_uses_the_scratch_default_and_is_check_only() {
    let backend = backend_for("elvis", plain_tool(), elvis_defaults);
    let findings = backend
        .diagnose("elvis", "lint", &single("lib/src/sub/a.erl", ELVIS_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "elvis");
    assert_eq!(findings[0].rule_id, "no_debug_call");
    assert_eq!(findings[0].path, "lib/src/sub/a.erl");
    assert!(backend
        .diagnose("elvis", "lint", &single("lib/src/a.erl", "-module(a).\n"))
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("elvis", "lib/src/a.erl", ELVIS_DIRTY, "lint")
            .expect("check-only"),
        ELVIS_DIRTY
    );
}

#[test]
fn elvis_bound_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("cfg/elvis.config".to_owned()),
        tool_files: vec![(
            "cfg/elvis.config".to_owned(),
            b"[{config, [#{files => [\"**/*.erl\"], ruleset => erl_files}]}].\n".to_vec(),
        )],
        ..plain_tool()
    };
    let backend = backend_for("elvis", tool, elvis_bound);
    let findings = backend
        .diagnose("elvis", "lint", &single("src/a.erl", ELVIS_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "src/a.erl");
}

const REDOCLY_ROOT: &str = "openapi: 3.1.0\ninfo:\n  title: Pets\n  version: '1'\npaths: {}\n";

fn redocly_files(root: &str) -> BTreeMap<String, String> {
    single("api/openapi.yaml", root)
}

#[test]
fn redocly_reports_root_findings_and_never_fixes() {
    let backend = backend_for("redocly", plain_tool(), roundtrip_redocly);
    assert!(backend
        .diagnose("redocly", "lint", &redocly_files(REDOCLY_ROOT))
        .expect("clean")
        .is_empty());
    let dupe = format!("{REDOCLY_ROOT}# dupe\n");
    let findings = backend
        .diagnose("redocly", "lint", &redocly_files(&dupe))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "redocly");
    assert_eq!(findings[0].rule_id, "operation-operationId-unique");
    assert_eq!(findings[0].path, "api/openapi.yaml");
    assert_eq!(findings[0].start_byte, Some(56));
    assert_eq!(
        backend
            .apply_fix("redocly", "api/openapi.yaml", &dupe, "lint")
            .expect("check-only"),
        dupe
    );
}

#[test]
fn redocly_places_fragment_findings_on_the_root() {
    let backend = backend_for("redocly", plain_tool(), roundtrip_redocly);
    let root = format!(
        "{REDOCLY_ROOT}components:\n  schemas:\n    Pet:\n      $ref: schemas/pet.yaml#/Pet\n# fragbad\n"
    );
    let fragment = single("api/schemas/pet.yaml", "Pet:\n  type: 7\n");
    let findings = backend
        .diagnose_with_siblings("redocly", "lint", &redocly_files(&root), &fragment)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "api/openapi.yaml");
    assert_eq!(findings[0].start_byte, Some(0));
    assert!(
        findings[0]
            .message
            .starts_with("api/schemas/pet.yaml:2:3: "),
        "{}",
        findings[0].message
    );
    let findings = backend
        .diagnose_with_resolve(
            "redocly",
            "lint",
            &redocly_files(&root),
            &BTreeMap::new(),
            &fragment,
        )
        .expect("diagnosed through resolves");
    assert_eq!(findings.len(), 1);
}

#[test]
fn redocly_preflight_findings_skip_the_tool() {
    let backend = backend_for("redocly", plain_tool(), roundtrip_redocly);
    for (root, rule) in [
        ("swagger: '2.0'\n", "dx/api-spec-root"),
        ("info: {}\n", "dx/api-spec-root"),
        ("openapi: [3\n", "dx/api-spec-syntax"),
        (
            "openapi: 3.1.0\nx:\n  $ref: https://example.com/a.yaml\n",
            "dx/api-spec-remote-ref",
        ),
        (
            "openapi: 3.1.0\nx:\n  $ref: missing.yaml\n",
            "dx/api-spec-unresolved-ref",
        ),
    ] {
        let findings = backend
            .diagnose(
                "redocly",
                "lint",
                &redocly_files(&format!("{root}# BROKEN\n")),
            )
            .expect("preflight findings");
        assert_eq!(findings.len(), 1, "{root}");
        assert_eq!(findings[0].rule_id, rule, "{root}");
        assert_eq!(findings[0].path, "api/openapi.yaml");
    }
}

#[test]
fn redocly_rejects_unsafe_configs_before_running() {
    for config in [
        "plugins:\n  - ./evil.js\n",
        "resolve:\n  http:\n    headers: []\n",
        "extends:\n  - ./other.yaml\n",
    ] {
        let tool = RealTool {
            config_rel: Some("api/redocly.yaml".to_owned()),
            tool_files: vec![("api/redocly.yaml".to_owned(), config.as_bytes().to_vec())],
            ..plain_tool()
        };
        let backend = backend_for("redocly", tool, roundtrip_redocly);
        let err = backend
            .diagnose("redocly", "lint", &redocly_files(REDOCLY_ROOT))
            .expect_err("unsafe config fails");
        assert!(matches!(err, RunnerError::ToolExecution { .. }), "{err}");
    }
}

#[test]
fn redocly_config_errors_are_never_clean() {
    let backend = backend_for("redocly", plain_tool(), roundtrip_redocly);
    let err = backend
        .diagnose(
            "redocly",
            "lint",
            &redocly_files(&format!("{REDOCLY_ROOT}# BROKEN\n")),
        )
        .expect_err("exit 2 fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("exit 2"), "{err}");
    assert!(err.to_string().contains("Unknown rule"), "{err}");
}

#[test]
fn redocly_defaults_are_builtin_and_offline() {
    let text = std::str::from_utf8(REDOCLY_DEFAULTS_BYTES).expect("utf-8");
    assert_eq!(
        quality_adapter::api_spec_preflight::check_config(text),
        Ok(())
    );
    assert!(text.contains("telemetry: off"));
    assert!(text.contains("no-unresolved-refs: error"));
}

type SpawnLog = std::sync::Mutex<Vec<(bool, PathBuf, usize)>>;

fn file_args(argv: &[OsString], extension: &str) -> Vec<PathBuf> {
    argv.iter()
        .skip(1)
        .map(PathBuf::from)
        .filter(|path| path.extension().is_some_and(|ext| ext == extension) && path.is_file())
        .collect()
}

fn trimmed(bytes: &[u8]) -> Vec<u8> {
    let mut fixed = Vec::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let trailing = line.ends_with(b"\n");
        let body = if trailing {
            &line[..line.len() - 1]
        } else {
            line
        };
        fixed.extend_from_slice(trim_end(body));
        if trailing {
            fixed.push(b'\n');
        }
    }
    fixed
}

fn ok_output(code: i32, stdout: String) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

fn multi_rustfmt(log: &SpawnLog, argv: &[OsString], cwd: &Path) -> io::Result<ChildOutput> {
    let files = file_args(argv, "rs");
    let check = argv.iter().any(|arg| arg == "--check");
    log.lock()
        .expect("log")
        .push((check, cwd.to_path_buf(), files.len()));
    let mut stdout = String::new();
    for file in &files {
        let bytes = std::fs::read(file).expect("file is materialized");
        let fixed = trimmed(&bytes);
        if fixed != bytes {
            if check {
                stdout.push_str(&format!("Diff in {}:1:\n-x  \n+x\n", file.display()));
            } else {
                std::fs::write(file, fixed).expect("fix writes back");
            }
        }
    }
    ok_output(i32::from(!stdout.is_empty()), stdout)
}

static RUSTFMT_SESSION_LOG: SpawnLog = std::sync::Mutex::new(Vec::new());

fn session_rustfmt(
    argv: &[OsString],
    cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    multi_rustfmt(&RUSTFMT_SESSION_LOG, argv, cwd)
}

#[test]
fn rustfmt_fix_spawns_once_per_stage_per_round_in_one_scratch() {
    let names: Vec<String> = (0..20).map(|index| format!("src/m{index:02}.rs")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let stages = vec![stage("rustfmt", &["rust"], &refs)];
    let files: Vec<crate::FileInput> = names
        .iter()
        .enumerate()
        .map(|(index, name)| file(name, if index == 7 { "x  \n" } else { "x\n" }))
        .collect();
    let backend = backend_for("rustfmt", rustfmt_tool(), session_rustfmt);
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("pipeline runs");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.replacements.len(), 1);
    let log = RUSTFMT_SESSION_LOG.lock().expect("log").clone();
    let shape: Vec<(bool, usize)> = log
        .iter()
        .map(|(check, _, count)| (*check, *count))
        .collect();
    assert_eq!(shape, vec![(true, 20), (false, 20), (false, 1), (true, 20)]);
    assert_eq!(log[1].1, log[2].1, "fix spawns share one scratch");
    assert_ne!(log[0].1, log[1].1, "diagnose keeps its own scratch");
}

static SHFMT_FALLBACK_SPAWNS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn partial_shfmt(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    SHFMT_FALLBACK_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let files = file_args(argv, "sh");
    let broken = files.iter().any(|file| {
        std::fs::read_to_string(file)
            .expect("file is materialized")
            .contains("BROKEN")
    });
    for file in &files {
        let bytes = std::fs::read(file).expect("file is materialized");
        let written = if broken {
            b"PARTIAL\n".to_vec()
        } else {
            trimmed(&bytes)
        };
        std::fs::write(file, written).expect("fix writes back");
    }
    ok_output(i32::from(broken), String::new())
}

fn sh_stage(count: usize, pad: usize) -> (crate::StageSpec, BTreeMap<String, String>) {
    let mut current = BTreeMap::new();
    for index in 0..count {
        current.insert(
            format!("bin/s{index:04}{}.sh", "s".repeat(pad)),
            "echo  \n".to_owned(),
        );
    }
    let refs: Vec<&str> = current.keys().map(String::as_str).collect();
    (stage("shfmt", &["shell"], &refs), current)
}

#[test]
fn batch_fix_failure_falls_back_to_single_files() {
    SHFMT_FALLBACK_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let (spec, mut current) = sh_stage(4, 0);
    let broken = spec.source_paths[2].clone();
    current.insert(broken.clone(), "BROKEN  \n".to_owned());
    let backend = backend_for("shfmt", plain_tool(), partial_shfmt);
    let known = BTreeSet::new();
    let mut session = super::real_fix::FixSession::new(&backend, "format", &known);
    let fixed = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect("fallback fixes");
    session.close().expect("session closes");
    assert_eq!(
        SHFMT_FALLBACK_SPAWNS.load(std::sync::atomic::Ordering::SeqCst),
        5
    );
    for (path, text) in &fixed {
        let want = if *path == broken {
            "BROKEN  \n"
        } else {
            "echo\n"
        };
        assert_eq!(text, want, "{path}");
    }
    assert_eq!(fixed.len(), 4);
}

static SHFMT_BUDGET_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn budget_shfmt(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    SHFMT_BUDGET_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_within_budget(argv);
    partial_shfmt(argv, cwd, env)
}

#[test]
fn large_fix_selection_splits_within_argv_budget() {
    SHFMT_BUDGET_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let (spec, current) = sh_stage(300, 120);
    let backend = backend_for("shfmt", plain_tool(), budget_shfmt);
    let known = BTreeSet::new();
    let mut session = super::real_fix::FixSession::new(&backend, "format", &known);
    let fixed = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect("batches fix");
    session.close().expect("session closes");
    let spawns = SHFMT_BUDGET_SPAWNS.load(std::sync::atomic::Ordering::SeqCst);
    assert!(spawns > 1 && spawns < 300, "{spawns} spawns");
    assert_eq!(fixed.len(), 300);
    assert!(fixed.values().all(|text| text == "echo\n"));
}

fn never_spawns(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    panic!("unexpected spawn: {argv:?}");
}

fn assert_keeps_without_scratch(tool_id: &str, tool: RealTool, capability: &str) {
    let parent = tempfile::tempdir().expect("scratch parent");
    let backend = RealBackend {
        tools: BTreeMap::from([(tool_id.to_owned(), tool)]),
        scratch_parent: parent.path().to_path_buf(),
        spawn: never_spawns,
        argv_limit: quality_adapter::commands::windows_argv_limit,
    };
    let spec = stage(tool_id, &["any"], &["a.txt", "b.txt"]);
    let current = BTreeMap::from([
        ("a.txt".to_owned(), "a  \n".to_owned()),
        ("b.txt".to_owned(), "b  \n".to_owned()),
    ]);
    let known = BTreeSet::new();
    let mut session = super::real_fix::FixSession::new(&backend, capability, &known);
    let fixed = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect("kept");
    session.close().expect("session closes");
    assert_eq!(fixed, current, "{tool_id} keeps its input");
    let left = std::fs::read_dir(parent.path())
        .expect("list parent")
        .count();
    assert_eq!(left, 0, "{tool_id} made no scratch");
}

#[test]
fn diagnose_only_stage_makes_no_fix_spawn() {
    assert_keeps_without_scratch("vale", plain_tool(), "lint");
    assert_keeps_without_scratch("shfmt", plain_tool(), "typecheck");
}

#[test]
fn format_only_tool_outside_format_makes_no_scratch() {
    assert_keeps_without_scratch("prettier", plain_tool(), "lint");
    let backend = backend_for("graphql_eslint", plain_tool(), never_spawns);
    let known = BTreeSet::new();
    let mut session = super::real_fix::FixSession::new(&backend, "lint", &known);
    let spec = stage("graphql_eslint", &["graphql"], &["a.graphql"]);
    let current = single("a.graphql", "type A\n");
    let err = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect_err("config is required");
    assert!(err.to_string().contains("requires a config"), "{err}");
}

fn statix_config_arg(argv: &[OsString]) -> PathBuf {
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("statix always gets --config");
    PathBuf::from(&argv[at + 1])
}

const STATIX_DIRTY: &str = "let\n  x = let in 1;\nin\nx\n";

fn statix_tool(argv: &[OsString], env: &[(OsString, OsString)]) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    assert_eq!(argv[argv.len() - 2], OsString::from("--"));
    let body = std::fs::read_to_string(&file).expect("read staged nix file");
    let dirty = body.contains("let in");
    if argv[0] == "fix" {
        if dirty {
            std::fs::write(&file, body.replace("let in ", "")).expect("rewrite nix file");
        }
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    assert_eq!(argv[..3], ["check", "-o", "json"].map(OsString::from));
    let stdout = if dirty {
        serde_json::json!({
            "file": file,
            "report": [{
                "note": "Useless let-in expression",
                "code": 2,
                "severity": "Warn",
                "diagnostics": [{
                    "at": {"from": {"line": 2, "column": 7}, "to": {"line": 2, "column": 15}},
                    "message": "This let-in expression has no entries",
                    "suggestion": null,
                }],
            }],
        })
        .to_string()
    } else {
        String::new()
    };
    Ok(ChildOutput {
        code: Some(i32::from(dirty)),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

fn statix_defaults(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let argv: Vec<OsString> = argv.iter().skip(1).cloned().collect();
    let config = statix_config_arg(&argv);
    assert!(
        config.ends_with("dx-statix-default/statix.toml"),
        "unbound statix uses the scratch default, got {config:?}"
    );
    let staged = std::fs::read(&config).expect("default statix config is staged");
    assert!(staged.is_empty());
    statix_tool(&argv, env)
}

fn statix_bound(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let argv: Vec<OsString> = argv.iter().skip(1).cloned().collect();
    let config = statix_config_arg(&argv);
    assert!(config.ends_with("cfg/statix.toml"), "{config:?}");
    let staged = std::fs::read(&config).expect("bound statix config is staged");
    assert_eq!(staged, b"disabled = [\"empty_pattern\"]\n");
    statix_tool(&argv, env)
}

fn statix_crash(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: b"config error: unable to parse config file\n".to_vec(),
    })
}

#[test]
fn statix_unbound_config_uses_the_scratch_default_and_fixes() {
    let backend = backend_for("statix", plain_tool(), statix_defaults);
    let findings = backend
        .diagnose("statix", "lint", &single("pkgs/a.nix", STATIX_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "statix");
    assert_eq!(findings[0].rule_id, "W02");
    assert_eq!(findings[0].path, "pkgs/a.nix");
    assert!(backend
        .diagnose("statix", "lint", &single("pkgs/a.nix", "1\n"))
        .expect("diagnosed")
        .is_empty());
    assert_eq!(
        backend
            .apply_fix("statix", "pkgs/a.nix", STATIX_DIRTY, "lint")
            .expect("fixed"),
        "let\n  x = 1;\nin\nx\n"
    );
}

#[test]
fn statix_bound_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("cfg/statix.toml".to_owned()),
        tool_files: vec![(
            "cfg/statix.toml".to_owned(),
            b"disabled = [\"empty_pattern\"]\n".to_vec(),
        )],
        ..plain_tool()
    };
    let backend = backend_for("statix", tool, statix_bound);
    let findings = backend
        .diagnose("statix", "lint", &single("a.nix", STATIX_DIRTY))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "W02");
}

#[test]
fn statix_errors_that_exit_zero_fail_the_action() {
    let backend = backend_for("statix", plain_tool(), statix_crash);
    backend
        .diagnose("statix", "lint", &single("a.nix", "1\n"))
        .expect_err("config error is never clean");
}

const DEADNIX_DIRTY: &str = "let\n  a = 1;\nin\n2\n";

fn deadnix_tool(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let at = argv
        .iter()
        .position(|arg| arg == "--")
        .expect("deadnix ends its flags");
    let files: Vec<String> = argv[at + 1..]
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert!(!files.is_empty());
    let edit = argv.iter().any(|arg| arg == "--edit");
    let mut lines = Vec::new();
    for file in &files {
        let body = std::fs::read_to_string(file).expect("read staged nix file");
        if !body.contains("a = 1;") {
            continue;
        }
        if edit {
            std::fs::write(file, body.replace("  a = 1;\n", "")).expect("rewrite nix file");
        } else {
            lines.push(
                serde_json::json!({
                    "file": file,
                    "results": [{"column": 3, "endColumn": 4, "line": 2, "message": "Unused let binding: a"}],
                })
                .to_string(),
            );
        }
    }
    Ok(ChildOutput {
        code: Some(i32::from(!lines.is_empty())),
        stdout: lines.join("\n").into_bytes(),
        stderr: Vec::new(),
    })
}

fn deadnix_garbage(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(1),
        stdout: b"a.nix: unused binding\n".to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn deadnix_reports_and_removes_dead_code() {
    let backend = backend_for("deadnix", plain_tool(), deadnix_tool);
    let mut files = single("a.nix", DEADNIX_DIRTY);
    files.insert("b.nix".to_owned(), "2\n".to_owned());
    let findings = backend
        .diagnose("deadnix", "lint", &files)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "deadnix");
    assert_eq!(findings[0].rule_id, "unused_let_binding");
    assert_eq!(findings[0].path, "a.nix");
    assert_eq!(
        backend
            .apply_fix("deadnix", "a.nix", DEADNIX_DIRTY, "lint")
            .expect("fixed"),
        "let\nin\n2\n"
    );
}

#[test]
fn deadnix_garbage_fails_the_action() {
    let backend = backend_for("deadnix", plain_tool(), deadnix_garbage);
    backend
        .diagnose("deadnix", "lint", &single("a.nix", DEADNIX_DIRTY))
        .expect_err("garbage is never clean");
}

const KUBECONFORM_CONFIG_REL: &str = "k8s/schemas.kubeconform.json";
const KUBECONFORM_MANIFEST: &str = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: a\n---\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\nspec:\n  replicas: \"2\"\n";

static KUBECONFORM_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn kubeconform_tool(config: &str) -> RealTool {
    RealTool {
        config_rel: Some(KUBECONFORM_CONFIG_REL.to_owned()),
        tool_files: vec![
            (
                KUBECONFORM_CONFIG_REL.to_owned(),
                config.as_bytes().to_vec(),
            ),
            (
                "k8s/schemas.kubeconform/core/pod-v1.json".to_owned(),
                b"{}".to_vec(),
            ),
        ],
        ..plain_tool()
    }
}

fn kubeconform_bound() -> RealTool {
    kubeconform_tool(
        r#"{"schema_version":1,"kubernetes_version":"1.31.0","strict":true,"skip_kinds":[],"core_dir":"schemas.kubeconform/core","crd_dir":"schemas.kubeconform/crd"}"#,
    )
}

fn kubeconform_fake(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    KUBECONFORM_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let args: Vec<String> = argv
        .iter()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args[..4],
        ["-output", "json", "-summary=false", "-kubernetes-version"]
    );
    assert!(!args
        .iter()
        .any(|arg| arg.contains("ignore-missing-schemas")));
    let locations: Vec<&String> = args
        .iter()
        .zip(args.iter().skip(1))
        .filter(|(flag, _)| *flag == "-schema-location")
        .map(|(_, value)| value)
        .collect();
    assert_eq!(locations.len(), 2);
    let core = locations[0]
        .strip_suffix("/{{.ResourceKind}}{{.KindSuffix}}.json")
        .expect("core template");
    assert!(
        Path::new(core).join("pod-v1.json").is_file(),
        "schemas are staged beside the config: {core}"
    );
    let docs: Vec<&String> = args
        .iter()
        .filter(|arg| arg.contains("dx-kubeconform"))
        .collect();
    assert!(!docs.is_empty());
    let mut resources = Vec::new();
    for doc in docs {
        let body = std::fs::read_to_string(doc).expect("read staged document");
        assert!(!body.contains("\n---"), "one document per file: {body:?}");
        if body.contains("replicas: \"2\"") {
            resources.push(serde_json::json!({
                "filename": doc,
                "kind": "Deployment",
                "name": "web",
                "version": "apps/v1",
                "status": "statusInvalid",
                "msg": format!("jsonschema: does not validate with file://{core}/deployment-apps-v1.json"),
                "validationErrors": [{"path": "/spec/replicas", "msg": "expected integer, but got string"}],
            }));
        }
    }
    Ok(ChildOutput {
        code: Some(i32::from(!resources.is_empty())),
        stdout: serde_json::json!({ "resources": resources })
            .to_string()
            .into_bytes(),
        stderr: Vec::new(),
    })
}

fn kubeconform_exit_zero_with_failures(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let mut out = kubeconform_fake(argv, cwd, env)?;
    out.code = Some(0);
    Ok(out)
}

#[test]
fn kubeconform_validates_every_document_in_one_run() {
    let backend = backend_for("kubeconform", kubeconform_bound(), kubeconform_fake);
    let mut files = single("k8s/app.yaml", KUBECONFORM_MANIFEST);
    files.insert("k8s/notes.yaml".to_owned(), "title: hi\n".to_owned());
    files.insert(
        "k8s/broken.yaml".to_owned(),
        "apiVersion: v1\nkind: [\n".to_owned(),
    );
    let before = KUBECONFORM_SPAWNS.load(std::sync::atomic::Ordering::SeqCst);
    let mut findings = backend
        .diagnose("kubeconform", "lint", &files)
        .expect("diagnosed");
    assert_eq!(
        KUBECONFORM_SPAWNS.load(std::sync::atomic::Ordering::SeqCst) - before,
        1
    );
    findings.sort_by(|a, b| a.path.cmp(&b.path));
    let seen: Vec<(&str, &str)> = findings
        .iter()
        .map(|found| (found.path.as_str(), found.rule_id.as_str()))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("k8s/app.yaml", "invalid"),
            ("k8s/broken.yaml", "dx/kubernetes-syntax"),
            ("k8s/notes.yaml", "dx/kubernetes-not-manifest"),
        ]
    );
    let invalid = &findings[0];
    assert_eq!(invalid.tool_id, "kubeconform");
    let replicas = KUBECONFORM_MANIFEST.find("replicas").expect("key") as u64;
    assert_eq!(invalid.start_byte, Some(replicas));
    assert!(!invalid.message.contains("file://"), "{}", invalid.message);
    assert_eq!(
        backend
            .apply_fix("kubeconform", "k8s/app.yaml", KUBECONFORM_MANIFEST, "lint")
            .expect("check-only"),
        KUBECONFORM_MANIFEST
    );
}

#[test]
fn kubeconform_clean_manifests_are_clean() {
    let backend = backend_for("kubeconform", kubeconform_bound(), kubeconform_fake);
    let findings = backend
        .diagnose(
            "kubeconform",
            "lint",
            &single("k8s/pod.yaml", "apiVersion: v1\nkind: Pod\n"),
        )
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
fn kubeconform_needs_a_local_config() {
    let backend = backend_for("kubeconform", plain_tool(), kubeconform_fake);
    let err = backend
        .diagnose(
            "kubeconform",
            "lint",
            &single("k8s/pod.yaml", "kind: Pod\n"),
        )
        .expect_err("no config");
    assert!(err.to_string().contains("kubernetes_schemas"), "{err}");
    let remote = kubeconform_tool(
        r#"{"schema_version":1,"kubernetes_version":"1.31.0","core_dir":"default","crd_dir":"c"}"#,
    );
    let backend = backend_for("kubeconform", remote, kubeconform_fake);
    let err = backend
        .diagnose(
            "kubeconform",
            "lint",
            &single("k8s/pod.yaml", "kind: Pod\n"),
        )
        .expect_err("remote schemas");
    assert!(err.to_string().contains("local directory"), "{err}");
}

#[test]
fn kubeconform_exit_zero_with_failures_fails_the_action() {
    let backend = backend_for(
        "kubeconform",
        kubeconform_bound(),
        kubeconform_exit_zero_with_failures,
    );
    backend
        .diagnose(
            "kubeconform",
            "lint",
            &single("k8s/app.yaml", KUBECONFORM_MANIFEST),
        )
        .expect_err("exit 0 with failures is never clean");
}

const NOTEBOOK_DIRTY: &str = "{\n  \"cells\": [\n    {\n      \"cell_type\": \"markdown\",\n      \"metadata\": {},\n      \"source\": [\"# Title\"]\n    },\n    {\n      \"cell_type\": \"code\",\n      \"execution_count\": 7,\n      \"metadata\": {\"tags\": [\"keep\"]},\n      \"outputs\": [{\"name\": \"stdout\", \"output_type\": \"stream\", \"text\": [\"hi\\n\"]}],\n      \"source\": [\n        \"import os\\n\",\n        \"print('hi')\"\n      ]\n    },\n    {\n      \"cell_type\": \"code\",\n      \"execution_count\": null,\n      \"metadata\": {},\n      \"outputs\": [],\n      \"source\": [\"x   =   1\"]\n    }\n  ],\n  \"metadata\": {\"language_info\": {\"name\": \"python\"}},\n  \"nbformat\": 4,\n  \"nbformat_minor\": 5\n}\n";

fn notebook_backend() -> RealBackend {
    backend_for("ruff", plain_tool(), ruff_notebook_behavior)
}

#[test]
fn ruff_notebook_findings_name_the_cell_and_file_line() {
    let backend = notebook_backend();
    let before = single("n.ipynb", NOTEBOOK_DIRTY);
    let findings = backend
        .diagnose("ruff", "lint", &before)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].message.starts_with("cell 2, line 1: "),
        "{}",
        findings[0].message
    );
    let os_at = NOTEBOOK_DIRTY.find("import os").expect("import") + "import ".len();
    assert_eq!(findings[0].start_byte, Some(os_at as u64));
    assert_eq!(before["n.ipynb"], NOTEBOOK_DIRTY, "check mode never writes");
}

#[test]
fn ruff_notebook_fix_splices_only_the_edited_cell() {
    let backend = notebook_backend();
    let fixed = backend
        .apply_fix("ruff", "n.ipynb", NOTEBOOK_DIRTY, "lint")
        .expect("fixed");
    let expected = NOTEBOOK_DIRTY.replace(
        "\"source\": [\n        \"import os\\n\",\n        \"print('hi')\"\n      ]",
        "\"source\": [\n        \"print('hi')\"\n      ]",
    );
    assert_eq!(fixed, expected);
    assert!(serde_json::from_str::<serde_json::Value>(&fixed).is_ok());
    assert!(backend
        .diagnose("ruff", "lint", &single("n.ipynb", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn ruff_notebook_batched_fix_handles_two_notebooks_and_a_python_file() {
    let backend = notebook_backend();
    let spec = stage(
        "ruff",
        &["jupyter", "python"],
        &["a.ipynb", "b.ipynb", "c.py"],
    );
    let current = BTreeMap::from([
        ("a.ipynb".to_owned(), NOTEBOOK_DIRTY.to_owned()),
        (
            "b.ipynb".to_owned(),
            NOTEBOOK_DIRTY.replace("x   =   1", "y = 2"),
        ),
        ("c.py".to_owned(), "import os\nx = 1\n".to_owned()),
    ]);
    let known = BTreeSet::new();
    let mut session = super::real_fix::FixSession::new(&backend, "lint", &known);
    let fixed = session
        .fix(0, &spec, &current, &spec.source_paths)
        .expect("fixed");
    session.close().expect("closes");
    assert_eq!(fixed["c.py"], "x = 1\n");
    for name in ["a.ipynb", "b.ipynb"] {
        assert!(!fixed[name].contains("import os"), "{name}");
        assert!(
            fixed[name].starts_with("{\n  \"cells\""),
            "{name} keeps its indent"
        );
        assert!(!fixed[name].contains("\"id\""), "{name} gains no ids");
    }
    assert!(fixed["b.ipynb"].contains("y = 2"));
}

#[test]
fn ruff_notebook_in_another_language_fails_instead_of_reporting_clean() {
    let backend = notebook_backend();
    let r_notebook = NOTEBOOK_DIRTY.replace(
        "{\"language_info\": {\"name\": \"python\"}}",
        "{\"kernelspec\": {\"language\": \"R\"}}",
    );
    let err = backend
        .diagnose("ruff", "lint", &single("n.ipynb", &r_notebook))
        .expect_err("rejected");
    assert!(err.to_string().contains("only Python notebooks"), "{err}");
    let err = backend
        .diagnose("ruff", "format", &single("n.ipynb", "{ not json"))
        .expect_err("rejected");
    assert!(err.to_string().contains("n.ipynb"), "{err}");
}

#[test]
fn notebooks_only_settle_through_ruff() {
    let err =
        RealBackend::settle("prettier", "n.ipynb", "{}", "{}".to_owned()).expect_err("refused");
    let text = err.to_string();
    assert!(
        text.contains("prettier") && text.contains("n.ipynb"),
        "{text}"
    );
    let err = RealBackend::settle("ruff", "n.ipynb", "{}", "{}".to_owned()).expect_err("splice");
    assert!(err.to_string().contains("n.ipynb"), "{err}");
    assert_eq!(
        RealBackend::settle("prettier", "a.md", "x", "y".to_owned()).expect("passes"),
        "y"
    );
}

const TYPST_DIRTY: &str = "#let   x = 1\n";
const TYPST_CLEAN: &str = "#let x = 1\n";
const TYPST_ERRONEOUS: &str = "#let x = (erroneous\n";

fn typstyle_tool(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(
        env.iter()
            .any(|(key, value)| key == "NO_COLOR" && value == "1"),
        "typstyle runs without colour"
    );
    let write = argv.iter().any(|arg| arg == "-i");
    assert!(write || argv.iter().any(|arg| arg == "--check"));
    assert!(!argv.iter().any(|arg| arg == "-q"), "-q hides the listing");
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut code = 0;
    for arg in argv
        .iter()
        .skip(1)
        .filter(|arg| !arg.to_string_lossy().starts_with('-'))
    {
        let path = Path::new(arg);
        let shown = path
            .strip_prefix(cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let body = std::fs::read_to_string(path).expect("read staged typst file");
        if body.contains("erroneous") {
            stderr.push_str(&format!(
                "warn: Failed to parse {shown}. The source is erroneous.\n"
            ));
        } else if body.contains("   ") {
            if write {
                std::fs::write(path, body.replace("   ", " ")).expect("rewrite typst file");
            } else {
                stdout.push_str(&format!("Would reformat: {shown}\n"));
                code = 1;
            }
        }
    }
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: stderr.into_bytes(),
    })
}

fn typstyle_unknown_exit(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(7),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn typstyle_junk(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(0),
        stdout: b"all good\n".to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn typstyle_check_reports_dirty_files_by_workspace_path() {
    let backend = backend_for("typstyle", plain_tool(), typstyle_tool);
    let mut files = single("docs/a.typ", TYPST_DIRTY);
    files.insert("docs/b.typ".to_owned(), TYPST_CLEAN.to_owned());
    let findings = backend
        .diagnose("typstyle", "format", &files)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "typstyle");
    assert_eq!(findings[0].path, "docs/a.typ");
    assert!(backend
        .diagnose("typstyle", "format", &single("docs/b.typ", TYPST_CLEAN))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn typstyle_apply_rewrites_the_owned_file_and_the_recheck_is_clean() {
    let backend = backend_for("typstyle", plain_tool(), typstyle_tool);
    let fixed = backend
        .apply_fix("typstyle", "docs/a.typ", TYPST_DIRTY, "format")
        .expect("fixed");
    assert_eq!(fixed, TYPST_CLEAN);
    assert!(backend
        .diagnose("typstyle", "format", &single("docs/a.typ", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn typstyle_syntax_error_at_exit_zero_is_never_clean() {
    let backend = backend_for("typstyle", plain_tool(), typstyle_tool);
    let findings = backend
        .diagnose("typstyle", "format", &single("a.typ", TYPST_ERRONEOUS))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "a.typ");
    // typstyle leaves the erroneous file as it was, so the recheck still reports it.
    let kept = backend
        .apply_fix("typstyle", "a.typ", TYPST_ERRONEOUS, "format")
        .expect("fix round completes");
    assert_eq!(kept, TYPST_ERRONEOUS);
    let again = backend
        .diagnose("typstyle", "format", &single("a.typ", &kept))
        .expect("diagnosed");
    assert_eq!(again.len(), 1);
}

#[test]
fn typstyle_unknown_exit_and_junk_output_are_never_clean() {
    for tool in [typstyle_unknown_exit as Spawn, typstyle_junk as Spawn] {
        let backend = backend_for("typstyle", plain_tool(), tool);
        backend
            .diagnose("typstyle", "format", &single("a.typ", TYPST_CLEAN))
            .expect_err("malformed output is never clean");
    }
}

const NIXFMT_CLEAN: &str = "{ a = 1; }\n";

/// A fake nixfmt that follows the real contract: `--check` never writes, a syntax error exits 1
/// with a located block on stderr and leaves the file alone, and a fix rewrites unformatted files.
fn fake_nixfmt(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let check = argv.iter().any(|arg| arg == "--check");
    let mut stderr = String::new();
    for file in argv.iter().skip(1).filter(|arg| *arg != "--check") {
        let name = file.to_string_lossy().into_owned();
        let text = std::fs::read_to_string(file)?;
        if text.contains("BROKEN") {
            stderr.push_str(&format!(
                "{name}:1:7:\n  |\n1 | {{ a = ; }}\n  |       ^^^^\nunexpected \";\"\nexpecting expression\n\n"
            ));
        } else if text != NIXFMT_CLEAN {
            if check {
                stderr.push_str(&format!("{name}: not formatted\n"));
            } else {
                std::fs::write(file, NIXFMT_CLEAN)?;
            }
        }
    }
    Ok(ChildOutput {
        code: Some(if stderr.is_empty() { 0 } else { 1 }),
        stdout: Vec::new(),
        stderr: stderr.into_bytes(),
    })
}

static TFLINT_SPAWNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn tflint_arg<'a>(argv: &'a [OsString], prefix: &str) -> Vec<&'a str> {
    argv.iter()
        .filter_map(|arg| arg.to_str()?.strip_prefix(prefix))
        .collect()
}

fn tflint_fake(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    TFLINT_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_hermetic(env);
    let value = |key: &str| {
        env.iter()
            .find(|(known, _)| known == key)
            .map(|(_, value)| PathBuf::from(value))
    };
    let plugins = value("TFLINT_PLUGIN_DIR").expect("plugin dir is pinned");
    assert!(value("HOME").is_none(), "HOME never reaches tflint");
    assert!(plugins.is_dir());
    assert_eq!(std::fs::read_dir(&plugins).expect("plugin dir").count(), 0);
    assert!(value("TFLINT_CONFIG_FILE").is_none());
    if cfg!(unix) {
        let tmp = value("TMPDIR").expect("TMPDIR is pinned");
        assert!(
            tmp.as_os_str().len() < 48,
            "socket-safe TMPDIR, got {tmp:?}"
        );
    }
    let at = argv
        .iter()
        .position(|arg| arg == "--config")
        .expect("tflint always gets --config");
    let config = PathBuf::from(&argv[at + 1]);
    assert!(
        config.ends_with("dx-tflint-default/.tflint.hcl"),
        "unbound tflint uses the scratch default, got {config:?}"
    );
    let text = std::fs::read_to_string(&config).expect("default config is staged");
    assert!(text.contains("call_module_type = \"none\""));
    assert!(text.contains("preset  = \"recommended\""));
    for flag in ["--format=json", "--no-color", "--call-module-type=none"] {
        assert!(argv.iter().any(|arg| arg == flag), "{flag} is passed");
    }
    let dir = tflint_arg(argv, "--chdir=")[0].to_owned();
    let filters = tflint_arg(argv, "--filter=");
    let issues: Vec<serde_json::Value> = filters
        .iter()
        .filter(|name| {
            std::fs::read_to_string(cwd.join(&dir).join(name))
                .expect("staged source")
                .contains("bad")
        })
        .map(|name| {
            serde_json::json!({
                "rule": {"name": "terraform_typed_variables", "severity": "warning", "link": ""},
                "message": "variable has no type",
                "range": {
                    "filename": format!("{dir}/{name}"),
                    "start": {"line": 1, "column": 1},
                    "end": {"line": 1, "column": 3},
                },
            })
        })
        .collect();
    let code = if issues.is_empty() { 0 } else { 2 };
    Ok(ChildOutput {
        code: Some(code),
        stdout: serde_json::json!({"issues": issues, "errors": []})
            .to_string()
            .into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
fn nixfmt_reports_a_dirty_file_and_fix_rewrites_it() {
    let backend = backend_for("nixfmt", plain_tool(), fake_nixfmt);
    let findings = backend
        .diagnose("nixfmt", "format", &single("a.nix", "{a=1;}\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "nixfmt");
    assert_eq!(findings[0].path, "a.nix");
    assert!(backend
        .diagnose("nixfmt", "format", &single("a.nix", NIXFMT_CLEAN))
        .expect("diagnosed")
        .is_empty());
    let fixed = backend
        .apply_fix("nixfmt", "a.nix", "{a=1;}\n", "format")
        .expect("fixed");
    assert_eq!(fixed, NIXFMT_CLEAN);
    assert!(backend
        .diagnose("nixfmt", "format", &single("a.nix", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn nixfmt_syntax_error_is_reported_and_a_fix_leaves_it_unchanged() {
    let backend = backend_for("nixfmt", plain_tool(), fake_nixfmt);
    let findings = backend
        .diagnose("nixfmt", "format", &single("b.nix", "BROKEN\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "b.nix");
    assert_eq!(
        backend
            .apply_fix("nixfmt", "b.nix", "BROKEN\n", "format")
            .expect("kept"),
        "BROKEN\n"
    );
}

#[test]
fn tflint_runs_once_per_directory_with_scratch_home_and_default_config() {
    TFLINT_SPAWNS.store(0, std::sync::atomic::Ordering::SeqCst);
    let backend = backend_for("tflint", plain_tool(), tflint_fake);
    let mut files = BTreeMap::new();
    files.insert("infra/a/main.tf".to_owned(), "bad\n".to_owned());
    files.insert("infra/a/vars.tf".to_owned(), "ok\n".to_owned());
    files.insert("infra/b/main.tf".to_owned(), "bad\n".to_owned());
    let findings = backend
        .diagnose("tflint", "lint", &files)
        .expect("diagnosed");
    assert_eq!(TFLINT_SPAWNS.load(std::sync::atomic::Ordering::SeqCst), 2);
    let mut paths: Vec<&str> = findings.iter().map(|f| f.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(paths, ["infra/a/main.tf", "infra/b/main.tf"]);
    assert_eq!(findings[0].tool_id, "tflint");
    assert_eq!(findings[0].rule_id, "terraform_typed_variables");
}

#[test]
fn tflint_is_check_only() {
    let backend = backend_for("tflint", plain_tool(), tflint_fake);
    assert_eq!(
        backend
            .apply_fix("tflint", "a/main.tf", "bad\n", "lint")
            .expect("check-only"),
        "bad\n"
    );
}

fn tflint_plugin_failure(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(1),
        stdout: br#"{"issues":[],"errors":[{"message":"Failed to initialize plugins; Plugin \"aws\" not found. Did you run \"tflint --init\"?","severity":"error"}]}"#.to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn tflint_plugin_failure_is_an_error_not_a_clean_run() {
    let backend = backend_for("tflint", plain_tool(), tflint_plugin_failure);
    let err = backend
        .diagnose("tflint", "lint", &single("a/main.tf", "ok\n"))
        .expect_err("never clean");
    assert!(err.to_string().contains("tflint --init"), "{err}");
}

const LATEX_DIRTY: &str = "\\begin{itemize}\n\\item one\n\\end{itemize}\n";
const LATEX_CLEAN: &str = "\\begin{itemize}\n\t\\item one\n\\end{itemize}\n";

fn latexindent_tool(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let write = argv.iter().any(|arg| arg == "-w");
    assert!(write || argv.iter().any(|arg| arg == "-k"));
    let flag = |prefix: &str| {
        argv.iter()
            .filter_map(|arg| arg.to_str()?.strip_prefix(prefix).map(PathBuf::from))
            .next()
    };
    let log = flag("-g=").expect("log goes to an explicit file");
    let cruft = flag("-c=").expect("backups go to an explicit directory");
    assert!(
        log.starts_with(cwd),
        "log {log:?} stays in the scratch tree"
    );
    assert!(
        cruft.starts_with(cwd),
        "cruft {cruft:?} stays in the scratch tree"
    );
    assert!(cruft.is_dir(), "the cruft directory exists");
    assert!(
        argv.iter().all(|arg| arg != "-l"),
        "a bare -l would auto-load settings from the source directory"
    );
    let path = Path::new(argv.last().expect("file argument"));
    let body = std::fs::read_to_string(path).expect("read staged latex file");
    let formatted = body.replace("\n\\item", "\n\t\\item");
    let mut code = 0;
    let mut stdout = String::new();
    if write {
        std::fs::write(path, &formatted).expect("rewrite latex file");
    } else if formatted != body {
        stdout = formatted;
        code = 1;
    } else {
        stdout = body;
    }
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

fn latexindent_with_config(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert!(
        argv.iter()
            .any(|arg| arg.to_str().is_some_and(|arg| arg.starts_with("-l="))),
        "a bound config is passed as -l=<file>"
    );
    latexindent_tool(argv, cwd, env)
}

fn latexindent_unknown_exit(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(7),
        stdout: LATEX_CLEAN.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

fn latexindent_fatal(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(3),
        stdout: b"FATAL: I couldn't find a.tex, are you sure it exists?\n".to_vec(),
        stderr: Vec::new(),
    })
}

fn latexindent_junk_stderr(
    _argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    Ok(ChildOutput {
        code: Some(0),
        stdout: LATEX_CLEAN.as_bytes().to_vec(),
        stderr: b"Can't locate YAML/Tiny.pm\n".to_vec(),
    })
}

#[test]
fn latexindent_check_reports_dirty_files_by_workspace_path() {
    let backend = backend_for("latexindent", plain_tool(), latexindent_tool);
    let mut files = single("docs/a.tex", LATEX_DIRTY);
    files.insert("docs/b.tex".to_owned(), LATEX_CLEAN.to_owned());
    let findings = backend
        .diagnose("latexindent", "format", &files)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "latexindent");
    assert_eq!(findings[0].path, "docs/a.tex");
    assert!(backend
        .diagnose("latexindent", "format", &single("docs/b.tex", LATEX_CLEAN))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn latexindent_apply_rewrites_the_owned_file_and_the_recheck_is_clean() {
    let backend = backend_for("latexindent", plain_tool(), latexindent_tool);
    let fixed = backend
        .apply_fix("latexindent", "docs/a.tex", LATEX_DIRTY, "format")
        .expect("fixed");
    assert_eq!(fixed, LATEX_CLEAN);
    assert!(backend
        .diagnose("latexindent", "format", &single("docs/a.tex", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn latexindent_passes_the_bound_config_and_keeps_log_and_backups_in_scratch() {
    let tool = RealTool {
        config_rel: Some("cfg/localSettings.yaml".to_owned()),
        ..plain_tool()
    };
    let backend = backend_for("latexindent", tool, latexindent_with_config);
    let mut files = single("docs/a.tex", LATEX_DIRTY);
    files.insert(
        "cfg/localSettings.yaml".to_owned(),
        "defaultIndent: \"  \"\n".to_owned(),
    );
    let findings = backend
        .diagnose("latexindent", "format", &files)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
}

#[test]
fn latexindent_unknown_exit_fatal_and_stderr_are_never_clean() {
    for tool in [
        latexindent_unknown_exit as Spawn,
        latexindent_fatal as Spawn,
        latexindent_junk_stderr as Spawn,
    ] {
        let backend = backend_for("latexindent", plain_tool(), tool);
        backend
            .diagnose("latexindent", "format", &single("a.tex", LATEX_CLEAN))
            .expect_err("malformed output is never clean");
    }
}

fn credo_fake(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], OsString::from("--config-file"));
    let config = PathBuf::from(&argv[2]);
    assert!(
        config.ends_with("dx-credo-default/.credo.exs"),
        "unbound credo uses the scratch default, got {config:?}"
    );
    let file = last_file(argv);
    assert!(
        !Path::new(&file).is_absolute(),
        "credo gets cwd-relative paths"
    );
    let text = std::fs::read_to_string(cwd.join(&file)).expect("read staged file");
    let (issues, invalid) = if text.contains("broken") {
        (
            String::new(),
            format!(r#"{{"path":"{file}","line":2,"column":8,"message":"missing terminator: )"}}"#),
        )
    } else if text.contains("IO.inspect") {
        (
            format!(
                r#"{{"category":"warning","check":"Credo.Check.Warning.IoInspect","column":3,"filename":"{file}","line_no":2,"message":"There should be no calls to `IO.inspect/1`."}}"#
            ),
            String::new(),
        )
    } else {
        (String::new(), String::new())
    };
    let code = i32::from(!issues.is_empty() || !invalid.is_empty());
    Ok(ChildOutput {
        code: Some(code),
        stdout: format!(r#"{{"issues":[{issues}]}}"#).into_bytes(),
        stderr: format!("dx-credo: {{\"invalid\":[{invalid}]}}\n").into_bytes(),
    })
}

#[test]
fn credo_reports_findings_as_warnings_with_the_default_config() {
    let backend = backend_for("credo", plain_tool(), credo_fake);
    let findings = backend
        .diagnose("credo", "lint", &single("lib/a.ex", "x\nIO.inspect(1)\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "lib/a.ex");
    assert_eq!(findings[0].rule_id, "credo/Warning.IoInspect");
    let clean = backend
        .diagnose("credo", "lint", &single("lib/a.ex", "ok\n"))
        .expect("clean");
    assert!(clean.is_empty());
}

#[test]
fn credo_reports_a_file_it_could_not_parse() {
    let backend = backend_for("credo", plain_tool(), credo_fake);
    let findings = backend
        .diagnose("credo", "lint", &single("lib/a.ex", "x\nbroken (\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "credo/syntax");
    assert_eq!(findings[0].message, "missing terminator: )");
}

#[test]
fn credo_lint_apply_leaves_bytes_unchanged() {
    let backend = backend_for("credo", plain_tool(), credo_fake);
    assert_eq!(
        backend
            .apply_fix("credo", "lib/a.ex", "IO.inspect(1)\n", "lint")
            .expect("check-only"),
        "IO.inspect(1)\n"
    );
}

fn outputs_backend() -> RealBackend {
    backend_for("notebook_outputs", plain_tool(), notebook_outputs_behavior)
}

fn outputs_stripped() -> String {
    NOTEBOOK_DIRTY
        .replace("\"execution_count\": 7", "\"execution_count\": null")
        .replace(
            "[{\"name\": \"stdout\", \"output_type\": \"stream\", \"text\": [\"hi\\n\"]}]",
            "[]",
        )
}

#[test]
fn notebook_outputs_findings_name_the_cell_and_never_write() {
    let backend = outputs_backend();
    let before = single("n.ipynb", NOTEBOOK_DIRTY);
    let findings = backend
        .diagnose("notebook_outputs", "lint", &before)
        .expect("diagnosed");
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.message.starts_with("cell 2: ")));
    let rules: Vec<&str> = findings.iter().map(|f| f.rule_id.as_str()).collect();
    assert_eq!(rules, ["execution-count", "cell-outputs"]);
    assert_eq!(before["n.ipynb"], NOTEBOOK_DIRTY, "check mode never writes");
}

#[test]
fn notebook_outputs_fix_strips_only_outputs_and_counts() {
    let backend = outputs_backend();
    let fixed = backend
        .apply_fix("notebook_outputs", "n.ipynb", NOTEBOOK_DIRTY, "lint")
        .expect("fixed");
    assert_eq!(fixed, outputs_stripped());
    assert!(backend
        .diagnose("notebook_outputs", "lint", &single("n.ipynb", &fixed))
        .expect("diagnosed")
        .is_empty());
}

#[test]
fn ruff_fix_then_notebook_outputs_fix_keeps_both_edits() {
    let after_ruff = notebook_backend()
        .apply_fix("ruff", "n.ipynb", NOTEBOOK_DIRTY, "lint")
        .expect("ruff fixed");
    let both = outputs_backend()
        .apply_fix("notebook_outputs", "n.ipynb", &after_ruff, "lint")
        .expect("outputs fixed");
    assert!(!both.contains("import os"));
    assert!(both.contains("\"execution_count\": null"));
    assert!(!both.contains("\"stdout\""));
}

#[test]
fn notebook_outputs_rewrite_that_touches_more_is_refused() {
    let backend = backend_for(
        "notebook_outputs",
        plain_tool(),
        notebook_outputs_tampering_behavior,
    );
    let err = backend
        .apply_fix("notebook_outputs", "n.ipynb", NOTEBOOK_DIRTY, "lint")
        .expect_err("refused");
    assert!(err.to_string().contains("n.ipynb"), "{err}");
}

#[test]
fn notebook_outputs_failures_never_report_clean() {
    let notebook = single("n.ipynb", NOTEBOOK_DIRTY);
    for behavior in [
        notebook_outputs_malformed_behavior as Spawn,
        notebook_outputs_crash_behavior,
        notebook_outputs_missing_behavior,
    ] {
        let backend = backend_for("notebook_outputs", plain_tool(), behavior);
        backend
            .diagnose("notebook_outputs", "lint", &notebook)
            .expect_err("a broken tool is an error");
    }
    let err = outputs_backend()
        .diagnose("notebook_outputs", "lint", &single("n.ipynb", "{ not json"))
        .expect_err("invalid JSON");
    assert!(err.to_string().contains("n.ipynb"), "{err}");
    let r_notebook = NOTEBOOK_DIRTY.replace(
        "{\"language_info\": {\"name\": \"python\"}}",
        "{\"kernelspec\": {\"language\": \"R\"}}",
    );
    let found = outputs_backend()
        .diagnose("notebook_outputs", "lint", &single("n.ipynb", &r_notebook))
        .expect("any kernel language is accepted");
    assert_eq!(found.len(), 2);
}

#[test]
fn notebook_outputs_settles_only_the_exact_strip() {
    let ok = RealBackend::settle(
        "notebook_outputs",
        "n.ipynb",
        NOTEBOOK_DIRTY,
        outputs_stripped(),
    )
    .expect("exact strip");
    assert_eq!(ok, outputs_stripped());
    RealBackend::settle(
        "notebook_outputs",
        "n.ipynb",
        NOTEBOOK_DIRTY,
        NOTEBOOK_DIRTY.replace("# Title", "# Other"),
    )
    .expect_err("any other rewrite is refused");
}

fn dialyzer_report(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let text: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let plts = text.iter().position(|a| a == "--plts").expect("plts flag");
    assert!(
        text[plts + 1].ends_with("dx-dialyzer/plt/000-core.plt"),
        "{text:?}"
    );
    assert!(
        text[plts + 2].ends_with("dx-dialyzer/plt/001-ext.plt"),
        "{text:?}"
    );
    assert_eq!(text[plts + 3], "--no_check_plt");
    let sep = text.iter().position(|a| a == "--").expect("separator");
    let beams = &text[sep + 1..];
    assert_eq!(beams.len(), 2, "{beams:?}");
    assert!(beams[0].contains("dx-dialyzer/owned/") && beams[0].ends_with("/a.beam"));
    assert!(beams[1].contains("dx-dialyzer/aux/") && beams[1].ends_with("/dep.beam"));
    Ok(ChildOutput {
        code: Some(2),
        stdout: b"  Proceeding with analysis...\na.erl:3:2: Function f/0 has no local return\ndep.erl:9:1: Function x/0 has no local return\nUnknown functions:\n  foo:bar/1 (a.erl:5:5)\n".to_vec(),
        stderr: Vec::new(),
    })
}

fn dialyzer_tool() -> RealTool {
    RealTool {
        tool_files: vec![
            ("dx-dialyzer/plt/001-ext.plt".to_owned(), b"e".to_vec()),
            ("dx-dialyzer/plt/000-core.plt".to_owned(), b"c".to_vec()),
            ("dx-dialyzer/aux/0/dep.beam".to_owned(), b"d".to_vec()),
            ("dx-dialyzer/owned/0/a.beam".to_owned(), b"a".to_vec()),
        ],
        ..plain_tool()
    }
}

const DIALYZER_SRC: &str = "-module(a).\n\nf() -> ok.\ng() ->\n    foo:bar(1), ok.\n";

#[test]
fn dialyzer_orders_plts_keeps_owned_findings_and_maps_basenames() {
    let backend = backend_for("dialyzer", dialyzer_tool(), dialyzer_report);
    let findings = backend
        .diagnose(
            "dialyzer",
            "typecheck",
            &single("lib/src/a.erl", DIALYZER_SRC),
        )
        .expect("diagnosed");
    let seen: Vec<(&str, &str)> = findings
        .iter()
        .map(|f| (f.path.as_str(), f.rule_id.as_str()))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("lib/src/a.erl", "no_return"),
            ("lib/src/a.erl", "unknown_function")
        ]
    );
}

#[test]
fn dialyzer_shared_basenames_resolve_to_the_first_sorted_path() {
    let backend = backend_for("dialyzer", dialyzer_tool(), dialyzer_report);
    let mut files = single("z/src/a.erl", DIALYZER_SRC);
    files.insert("b/src/a.erl".to_owned(), DIALYZER_SRC.to_owned());
    let findings = backend
        .diagnose("dialyzer", "typecheck", &files)
        .expect("diagnosed");
    assert!(
        findings.iter().all(|f| f.path == "b/src/a.erl"),
        "{findings:?}"
    );
}

fn dialyzer_elixir_report(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let text: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let pa = text
        .iter()
        .position(|a| a == "-pa")
        .expect("code path flag");
    assert!(text[pa + 1].ends_with("dx-dialyzer/pa"), "{text:?}");
    assert!(text.iter().any(|a| a == "--fullpath"), "{text:?}");
    let sep = text.iter().position(|a| a == "--").expect("separator");
    let beams = &text[sep + 1..];
    assert_eq!(beams.len(), 2, "{beams:?}");
    assert!(
        beams.iter().all(|b| b.ends_with(".beam")),
        "only beams are analysed: {beams:?}"
    );
    Ok(ChildOutput {
        code: Some(2),
        stdout: b"  Proceeding with analysis...\n/sandbox/pkg/lib/b/w.ex:3:2: Function f/0 has no local return\npkg/lib/a/w.ex:4:1: Function g/0 has no local return\n".to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn dialyzer_stages_owned_directories_and_reports_each_source_path() {
    let tool = RealTool {
        tool_files: vec![
            ("dx-dialyzer/plt/000-core.plt".to_owned(), b"c".to_vec()),
            ("dx-dialyzer/pa/elixir_erl.beam".to_owned(), b"p".to_vec()),
            (
                "dx-dialyzer/owned/0/ebin/Elixir.A.beam".to_owned(),
                b"a".to_vec(),
            ),
            ("dx-dialyzer/owned/0/ebin/a.app".to_owned(), b"app".to_vec()),
            (
                "dx-dialyzer/owned/0/ebin/Elixir.B.beam".to_owned(),
                b"b".to_vec(),
            ),
        ],
        ..plain_tool()
    };
    let backend = backend_for("dialyzer", tool, dialyzer_elixir_report);
    let mut files = single(
        "pkg/lib/a/w.ex",
        "defmodule A do\n\n\n  def g, do: 1\nend\n",
    );
    files.insert(
        "pkg/lib/b/w.ex".to_owned(),
        "defmodule B do\n\n  def f, do: 1\nend\n".to_owned(),
    );
    let findings = backend
        .diagnose("dialyzer", "typecheck", &files)
        .expect("diagnosed");
    let seen: Vec<&str> = findings.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(seen, vec!["pkg/lib/a/w.ex", "pkg/lib/b/w.ex"]);
}

#[test]
fn dialyzer_skips_an_app_without_beams() {
    let tool = RealTool {
        tool_files: vec![
            ("dx-dialyzer/plt/000-core.plt".to_owned(), b"c".to_vec()),
            ("dx-dialyzer/owned/0/ebin/a.app".to_owned(), b"app".to_vec()),
        ],
        ..plain_tool()
    };
    let backend = backend_for("dialyzer", tool, |_, _, _| {
        panic!("dialyzer must not run without beams")
    });
    let findings = backend
        .diagnose("dialyzer", "typecheck", &single("lib/a.ex", "x\n"))
        .expect("skipped");
    assert!(findings.is_empty());
}

#[test]
fn dialyzer_is_check_only() {
    let backend = backend_for("dialyzer", dialyzer_tool(), dialyzer_report);
    assert_eq!(
        backend
            .apply_fix("dialyzer", "lib/src/a.erl", DIALYZER_SRC, "typecheck")
            .expect("check-only"),
        DIALYZER_SRC
    );
}

fn phpstan_report(
    argv: &[OsString],
    _cwd: &Path,
    _env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let text: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let config = text.iter().position(|a| a == "-c").expect("config flag");
    let neon = std::fs::read_to_string(&text[config + 1]).expect("generated neon");
    assert!(neon.contains("maximumNumberOfProcesses: 1"), "{neon}");
    assert!(neon.contains("tmpDir"), "{neon}");
    assert!(neon.contains("dx-phpstan/scan"), "{neon}");
    assert!(neon.contains("dx-phpstan/pkg"), "{neon}");
    assert!(neon.contains("level: 5"), "{neon}");
    let sep = text.iter().position(|a| a == "--").expect("separator");
    let paths = &text[sep + 1..];
    assert_eq!(paths.len(), 1, "{paths:?}");
    let clean = paths[0].ends_with("clean.php");
    let body = if clean {
        r#"{"totals":{"errors":0,"file_errors":0},"files":[],"errors":[]}"#.to_owned()
    } else {
        format!(
            r#"{{"totals":{{"errors":0,"file_errors":2}},"files":{{"{0}":{{"errors":1,"messages":[{{"message":"Method f() should return int.","line":3,"ignorable":true,"identifier":"return.type"}}]}},"/elsewhere/dep.php":{{"errors":1,"messages":[{{"message":"x","line":1,"ignorable":true}}]}}}},"errors":[]}}"#,
            paths[0]
        )
    };
    Ok(ChildOutput {
        code: Some(i32::from(!clean)),
        stdout: body.into_bytes(),
        stderr: Vec::new(),
    })
}

fn phpstan_tool() -> RealTool {
    RealTool {
        tool_files: vec![
            (
                "dx-phpstan/scan/lib/Dep.php".to_owned(),
                b"<?php\n".to_vec(),
            ),
            (
                "dx-phpstan/pkg/vendor/x/X.php".to_owned(),
                b"<?php\n".to_vec(),
            ),
        ],
        ..plain_tool()
    }
}

#[test]
fn phpstan_writes_its_config_keeps_owned_findings_and_maps_paths_back() {
    let backend = backend_for("phpstan", phpstan_tool(), phpstan_report);
    let findings = backend
        .diagnose(
            "phpstan",
            "typecheck",
            &single("lib/bad.php", "<?php\nfunction f(): int { return 'a'; }\n"),
        )
        .expect("diagnosed");
    let seen: Vec<(&str, &str)> = findings
        .iter()
        .map(|f| (f.path.as_str(), f.rule_id.as_str()))
        .collect();
    assert_eq!(seen, vec![("lib/bad.php", "phpstan/return.type")]);
    assert!(backend
        .diagnose("phpstan", "typecheck", &single("lib/clean.php", "<?php\n"))
        .expect("clean")
        .is_empty());
}

#[test]
fn phpstan_is_check_only() {
    let backend = backend_for("phpstan", phpstan_tool(), phpstan_report);
    assert_eq!(
        backend
            .apply_fix("phpstan", "lib/a.php", "<?php\n", "typecheck")
            .expect("check-only"),
        "<?php\n"
    );
}

const ANSIBLE_CLEAN_SARIF: &str =
    r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"ansible-lint"}},"results":[]}]}"#;

fn ansible_clean(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_ne!(argv[1], "--dx-collections-path", "{argv:?}");
    Ok(ChildOutput {
        code: Some(0),
        stdout: ANSIBLE_CLEAN_SARIF.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

fn ansible_expects_external_root(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], "--dx-collections-path", "{argv:?}");
    let roots: Vec<PathBuf> = std::env::split_paths(&argv[2]).collect();
    assert_eq!(roots.len(), 1, "{roots:?}");
    assert!(roots[0].ends_with("_external/dx_hub"), "{roots:?}");
    assert!(
        roots[0]
            .join("ansible_collections/dxtest/tiny/MANIFEST.json")
            .is_file(),
        "collection files are staged under the root"
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: ANSIBLE_CLEAN_SARIF.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

fn ansible_collection_tool(files: &[&str]) -> RealTool {
    RealTool {
        tool_files: files
            .iter()
            .map(|rel| ((*rel).to_owned(), b"{}".to_vec()))
            .collect(),
        ..plain_tool()
    }
}

#[test]
fn ansible_lint_collection_roots_reach_argv_from_external_repos() {
    let tool = ansible_collection_tool(&[
        "_external/dx_hub/ansible_collections/dxtest/tiny/MANIFEST.json",
        "_external/dx_hub/ansible_collections/dxtest/tiny/plugins/modules/hello.py",
    ]);
    let backend = backend_for("ansible_lint", tool, ansible_expects_external_root);
    let findings = backend
        .diagnose("ansible_lint", "lint", &single("site.yml", "---\n"))
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
fn ansible_lint_without_collections_gets_no_flag() {
    let backend = backend_for("ansible_lint", plain_tool(), ansible_clean);
    let findings = backend
        .diagnose("ansible_lint", "lint", &single("site.yml", "---\n"))
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
fn ansible_lint_duplicate_collection_under_two_roots_is_an_error() {
    let tool = ansible_collection_tool(&[
        "_external/a/ansible_collections/dxtest/tiny/MANIFEST.json",
        "_external/b/ansible_collections/dxtest/tiny/MANIFEST.json",
    ]);
    let backend = backend_for("ansible_lint", tool, ansible_clean);
    let err = backend
        .diagnose("ansible_lint", "lint", &single("site.yml", "---\n"))
        .expect_err("duplicate roots must not be clean");
    assert!(err.to_string().contains("dxtest.tiny"), "{err}");
}

/// Stands in for `cue vet`: asserts the argv, the module-root cwd and the sealed environment.
fn cue_vet_fake(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let flags: Vec<String> = argv[1..4]
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(flags, ["vet", "-c=false", "-E"]);
    assert!(cwd.ends_with("app"), "{cwd:?}");
    let get = |key: &str| {
        env.iter()
            .find(|(known, _)| known == key)
            .map(|(_, value)| PathBuf::from(value))
    };
    let scratch = cwd.parent().expect("scratch root");
    assert!(get("CUE_CACHE_DIR").expect("cache").starts_with(scratch));
    assert!(get("CUE_CONFIG_DIR").expect("config").starts_with(scratch));
    for key in ["HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME"] {
        assert!(
            env.iter()
                .all(|(known, _)| !known.to_string_lossy().eq_ignore_ascii_case(key)),
            "{key} never reaches cue"
        );
    }
    let targets: Vec<String> = argv[4..]
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(targets.len(), 1, "one package per spawn: {targets:?}");
    assert!(
        targets[0] == ".:app" || targets[0] == "./lib:lib",
        "vetted by import path with package: {targets:?}"
    );
    let dir = targets[0].split(':').next().expect("dir");
    assert!(dir == "." || dir.starts_with("./"), "{targets:?}");
    let main = cwd.join(dir).join("main.cue");
    let text = std::fs::read_to_string(&main).unwrap_or_default();
    if text.contains("BAD") {
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"name: conflicting values 1 and string:\n    ./main.cue:1:1\n".to_vec(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn cue_module(main: &str) -> BTreeMap<String, String> {
    let mut files = single("app/main.cue", main);
    files.insert(
        "app/cue.mod/module.cue".to_owned(),
        "module: \"x.com/app\"\n".to_owned(),
    );
    files.insert("app/lib/g.cue".to_owned(), "package lib\n".to_owned());
    files
}

#[test]
fn cue_typecheck_vets_per_package_from_the_module_root() {
    let backend = backend_for("cue", plain_tool(), cue_vet_fake);
    let clean = backend
        .diagnose("cue", "typecheck", &cue_module("package app\n"))
        .expect("diagnosed");
    assert!(clean.is_empty(), "{clean:?}");
    let found = backend
        .diagnose("cue", "typecheck", &cue_module("package app\nBAD\n"))
        .expect("diagnosed");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, "app/main.cue");
}
