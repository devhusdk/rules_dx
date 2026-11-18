use super::*;

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
        quality_result::proto::Severity::Warning as i32
    );
    let fixed = backend
        .apply_fix("ruff", "a.py", "import os\n# UNFIXABLE\n", "lint")
        .expect("fixed");
    assert_eq!(fixed, "# UNFIXABLE\n");
    assert_eq!(
        backend
            .diagnose("ruff", "lint", &single("a.py", "x = 1\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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

fn ruff_in_config_dir(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert!(cwd.ends_with("services/api"), "{cwd:?}");
    roundtrip_ruff_hinted(argv, cwd, env)
}

fn ruff_at_scratch_root(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert!(!cwd.ends_with("services/api"), "{cwd:?}");
    roundtrip_ruff(argv, cwd, env)
}

#[test]
fn ruff_nested_config_runs_in_its_directory() {
    let tool = RealTool {
        config_rel: Some("services/api/ruff.toml".to_owned()),
        tool_files: vec![("services/api/ruff.toml".to_owned(), b"".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("ruff", tool, ruff_in_config_dir);
    let path = "services/api/tests/test_app.py";
    let lint = backend
        .diagnose("ruff", "lint", &single(path, "import os\n"))
        .expect("diagnosed");
    assert_eq!(lint.len(), 1);
    let format = backend
        .diagnose("ruff", "format", &single(path, "x = 1  \n"))
        .expect("diagnosed");
    assert_eq!(format.len(), 1);
    let fixed = backend
        .apply_fix("ruff", path, "import os\nx = 1\n", "lint")
        .expect("fixed");
    assert_eq!(fixed, "x = 1\n");
    let fixed = backend
        .apply_fix("ruff", path, "x = 1  \n", "format")
        .expect("fixed");
    assert_eq!(fixed, "x = 1\n");
}

#[test]
fn ruff_unhinted_runs_at_scratch_root() {
    let backend = backend_for("ruff", plain_tool(), ruff_at_scratch_root);
    backend
        .diagnose("ruff", "lint", &single("services/api/a.py", "x = 1\n"))
        .expect("diagnosed");
}

#[test]
fn ruff_cwd_is_the_config_directory_unless_external() {
    assert_eq!(
        RealBackend::cwd_rel("ruff", Some("services/api/ruff.toml")),
        "services/api"
    );
    assert_eq!(
        RealBackend::cwd_rel("ruff", Some("_external/r/ruff.toml")),
        ""
    );
    assert_eq!(RealBackend::cwd_rel("ruff", None), "");
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
    assert_eq!(
        backend
            .diagnose("ty", "typecheck", &single("a.py", "x: int = 1\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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
    assert_eq!(
        backend
            .diagnose(
                "pydoclint",
                "lint",
                &single("a.py", "\"\"\"Module.\"\"\"\n")
            )
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
    let text = "def foo():\n    NODOC\n";
    assert_eq!(
        backend
            .apply_fix("pydoclint", "a.py", text, "lint")
            .expect("check-only"),
        text
    );
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
    assert_eq!(
        backend
            .diagnose("ruff", "format", &single("a.py", "x = 1\n"))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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

static RUFF_SPAWNS: SpawnCount = SpawnCount::new();

fn batched_ruff(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    RUFF_SPAWNS.bump();
    assert_within_budget(argv);
    ruff_multi_behavior(argv, cwd, env)
}

static RUFF_EQUAL_SPAWNS: SpawnCount = SpawnCount::new();

fn batched_ruff_equal(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    RUFF_EQUAL_SPAWNS.bump();
    assert_within_budget(argv);
    ruff_multi_behavior(argv, cwd, env)
}

static LAUNCHER_SPAWNS: SpawnCount = SpawnCount::new();

fn batched_launcher(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    LAUNCHER_SPAWNS.bump();
    assert_eq!(
        commands::argv_budget(&argv[0]),
        commands::LAUNCHER_BATCH_BUDGET
    );
    assert_within_budget(argv);
    ruff_multi_behavior(argv, cwd, env)
}

static TY_SPAWNS: SpawnCount = SpawnCount::new();

fn counting_ty(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    TY_SPAWNS.bump();
    roundtrip_ty(argv, cwd, env)
}

#[test]
fn ruff_large_selection_runs_batched() {
    let files = python_selection(300, 96);
    for (capability, kind) in [("lint", "F401"), ("format", "unformatted")] {
        RUFF_SPAWNS.reset();
        let backend = backend_for("ruff", plain_tool(), batched_ruff);
        let first = backend
            .diagnose("ruff", capability, &files)
            .expect("batched ruff runs");
        let spawns = RUFF_SPAWNS.get();
        assert!(spawns > 1, "{capability} splits into {spawns} spawns");
        assert_eq!(first.len(), 150, "every other file is dirty");
        assert!(
            first.iter().all(|found| found.rule_id.contains(kind)),
            "{capability}"
        );
        let paths: Vec<&str> = first.iter().map(|found| found.path.as_str()).collect();
        let mut ordered = paths.clone();
        ordered.sort_unstable();
        assert_eq!(paths, ordered, "merged findings stay ordered");
        let second = backend
            .diagnose("ruff", capability, &files)
            .expect("batched ruff reruns");
        assert_eq!(first, second, "rerun merges identically");
    }
}

#[test]
fn ruff_batches_count_the_source_roots_they_add() {
    let mut files = BTreeMap::new();
    for index in 0..200 {
        files.insert(
            format!("dir_{index:04}_{}/mod.py", "d".repeat(60)),
            "import os \n".to_owned(),
        );
    }
    RUFF_SPAWNS.reset();
    let backend = backend_for("ruff", plain_tool(), batched_ruff);
    let found = backend
        .diagnose("ruff", "lint", &files)
        .expect("batched ruff runs");
    assert!(RUFF_SPAWNS.get() > 1);
    assert_eq!(found.len(), 200);
}

#[test]
fn ruff_batches_match_single_file_results() {
    let files = python_selection(120, 200);
    RUFF_EQUAL_SPAWNS.reset();
    let batched = backend_for("ruff", plain_tool(), batched_ruff_equal);
    let merged = batched
        .diagnose("ruff", "lint", &files)
        .expect("batched ruff runs");
    assert!(
        RUFF_EQUAL_SPAWNS.get() > 1,
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
fn ty_over_budget_selection_fails_before_spawn() {
    TY_SPAWNS.reset();
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
    assert_eq!(TY_SPAWNS.get(), 0);
}

#[test]
fn batch_launcher_splits_at_launcher_budget() {
    LAUNCHER_SPAWNS.reset();
    let files = python_selection(120, 96);
    let tool = RealTool {
        binary: PathBuf::from("/fake/bin/ruff.bat"),
        ..plain_tool()
    };
    let backend = backend_for("ruff", tool, batched_launcher);
    let findings = backend
        .diagnose("ruff", "lint", &files)
        .expect("launcher batches stay under the guard");
    assert!(LAUNCHER_SPAWNS.get() > 2);
    assert_eq!(findings.len(), 60);
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
    assert_eq!(
        backend
            .diagnose("ruff", "lint", &single("n.ipynb", &fixed))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
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
    let mut session = real_fix::FixSession::new(&backend, "lint", &known);
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
