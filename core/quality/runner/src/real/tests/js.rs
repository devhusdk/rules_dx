use super::*;

#[test]
fn biome_lint_runs_with_the_bound_config() {
    let backend = backend_for("biome", bound_tool("cfg/biome.json", b"{}"), biome_defaults);
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
    assert_eq!(
        backend
            .diagnose("biome", "lint", &single("src/a.js", "const x = 1;\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
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
    let backend = backend_for(
        "biome",
        bound_tool("cfg/biome.json", b"{}"),
        roundtrip_biome,
    );
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
    assert_eq!(
        backend
            .diagnose("biome", "format", &single("src/a.js", "const x = 1;\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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
    assert_eq!(
        backend
            .diagnose("eslint", "lint", &single("src/a.js", "const x = 1;\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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
    assert!(err.to_string().contains("invariant"));
    let err = backend
        .apply_fix("eslint", "src/a.js", "const x = 1;\n", "lint")
        .expect_err("missing config fails");
    assert!(err.to_string().contains("invariant"));
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
    assert_eq!(
        backend
            .diagnose(
                "graphql_eslint",
                "lint",
                &single("api/query.graphql", "{ user { id } }\n")
            )
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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
    assert!(err.to_string().contains("invariant"));
    let err = backend
        .apply_fix("graphql_eslint", "api/q.graphql", "{ a }\n", "lint")
        .expect_err("missing config fails");
    assert!(err.to_string().contains("invariant"));
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
    assert_eq!(
        backend
            .diagnose("prettier", "format", &single("src/a.js", "const x = 1;\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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
    let backend = backend_for("biome", bound_tool("cfg/biome.json", b"{}"), failing_fix);
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
    let words: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let bound: Vec<&str> = words
        .iter()
        .filter_map(|arg| arg.strip_prefix("--dx-config="))
        .collect();
    assert_eq!(bound.len(), 1, "hinted config passes one --dx-config");
    for reserved in ["--config", "--no-config", "--ignore-path"] {
        assert!(
            !words
                .iter()
                .any(|arg| arg == reserved || arg.starts_with(&format!("{reserved}="))),
            "{reserved} is owned by the prettier wrapper"
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
    assert_eq!(findings, [] as [quality_result::proto::Diagnostic; 0]);
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
    assert_eq!(findings, [] as [quality_result::proto::Diagnostic; 0]);
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
