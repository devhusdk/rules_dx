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
    _cwd: &Path,
    _env: &[(OsString, OsString)],
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
    assert!(cfg.ends_with("cfg/.prettierrc.json"));
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
            parsers::parse_ruff(&ruff.stdout, Some(1), &[absolute.as_str()]),
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
