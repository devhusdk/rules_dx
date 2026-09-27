use super::real_core::*;
use super::real_fixtures::*;
use super::*;
use crate::run_convergence;
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
fn nonzero_fix_keeps_the_input_bytes() {
    let backend = backend_for("rustfmt", rustfmt_tool(), failing_fix);
    let kept = backend
        .apply_fix("rustfmt", "src/main.rs", "x  \n", "format")
        .expect("kept");
    assert_eq!(kept, "x  \n");
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, agree).expect("converged");
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
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, normalize).expect("converged");
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
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, normalize).expect("converged");
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
    env: &[(String, String)],
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

fn ty_garbage(argv: &[OsString], _cwd: &Path, env: &[(String, String)]) -> io::Result<ChildOutput> {
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
