use super::*;

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
    assert_eq!(
        backend
            .diagnose(
                "keep_sorted",
                "lint",
                &single("notes.txt", KEEP_SORTED_CLEAN)
            )
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
    let fixed = backend
        .apply_fix("keep_sorted", "notes.txt", KEEP_SORTED_DIRTY, "lint")
        .expect("fixed");
    assert_eq!(fixed, KEEP_SORTED_CLEAN);
    assert_eq!(
        backend
            .diagnose("keep_sorted", "lint", &single("notes.txt", &fixed))
            .expect("diagnosed"),
        [] as [quality_result::proto::Diagnostic; 0]
    );
}

#[test]
fn keep_sorted_lint_pipeline_fixes_dirty_files_to_stable() {
    let backend = backend_for("keep_sorted", plain_tool(), keep_sorted_behavior);
    let stages = vec![stage("keep_sorted", &["text"], &["notes.txt"])];
    let files = vec![file("notes.txt", KEEP_SORTED_DIRTY)];
    let result = run_real_pipeline("//core/quality:test", "lint", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert_eq!(result.initial_diagnostics[0].tool_id, "keep_sorted");
    assert_eq!(
        result.terminal_diagnostics,
        [] as [quality_result::proto::Diagnostic; 0]
    );
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

fn batched_keep_sorted_spawns() -> usize {
    BATCHED_SPAWNS.get()
}

static BATCHED_SPAWNS: SpawnCount = SpawnCount::new();

fn batched_keep_sorted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    BATCHED_SPAWNS.bump();
    assert_within_budget(argv);
    keep_sorted_multi_behavior(argv, cwd, env)
}

static FLAKY_SPAWNS: SpawnCount = SpawnCount::new();

fn flaky_keep_sorted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let at = FLAKY_SPAWNS.bump();
    if at == 1 {
        return Err(io::Error::other("terminal gone"));
    }
    keep_sorted_multi_behavior(argv, cwd, env)
}

static POISONED_SPAWNS: SpawnCount = SpawnCount::new();

fn poisoned_keep_sorted(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    let at = POISONED_SPAWNS.bump();
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
    BATCHED_SPAWNS.reset();
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
    ordered.sort_unstable();
    assert_eq!(paths, ordered, "merged findings stay ordered");
    let second = backend
        .diagnose("keep_sorted", "lint", &files)
        .expect("batched check reruns");
    assert_eq!(first, second, "rerun merges identically");
}

#[test]
fn keep_sorted_batches_match_single_file_results() {
    let files = padded_selection(120, 200);
    BATCHED_SPAWNS.reset();
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
    merged_paths.sort_unstable();
    single_paths.sort_unstable();
    assert_eq!(merged_paths, single_paths);
    assert_eq!(merged.len(), 60);
}

#[test]
fn keep_sorted_middle_batch_spawn_failure_aborts() {
    FLAKY_SPAWNS.reset();
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
    POISONED_SPAWNS.reset();
    let files = oversized_selection(300);
    let backend = backend_for("keep_sorted", plain_tool(), poisoned_keep_sorted);
    let err = backend
        .diagnose("keep_sorted", "lint", &files)
        .expect_err("middle batch fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
    assert!(err.to_string().contains("batch 2/"), "{err}");
}
