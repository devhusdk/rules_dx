use super::real_core::*;
use super::real_fixtures::*;
use super::*;
use crate::run_convergence;
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
        file.ends_with("src dir/my file.js"),
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
