use super::*;
use quality_result::{decode_validated, encode_validated, validate};

fn stage(tool: &str, classes: &[&str], sources: &[&str]) -> StageSpec {
    StageSpec {
        tool_id: tool.to_owned(),
        class_ids: classes.iter().map(ToString::to_string).collect(),
        source_paths: sources.iter().map(ToString::to_string).collect(),
    }
}

fn file(path: &str, body: &str) -> FileInput {
    FileInput {
        path: path.to_owned(),
        bytes: body.as_bytes().to_vec(),
    }
}

fn lint_fix() -> (Vec<StageSpec>, Vec<FileInput>) {
    (
        vec![stage("lint-a", &["rust"], &["src/lib.rs"])],
        vec![file("src/lib.rs", "BAD BAD\n")],
    )
}

#[test]
fn stable_fix_is_valid_and_deterministic() {
    let (stages, files) = lint_fix();
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.terminal_snapshot.len(), 1);
    assert_ne!(
        result.original_snapshot[0].digest,
        result.terminal_snapshot[0].digest
    );
    assert_eq!(result.initial_diagnostics.len(), 2);
    assert!(result.terminal_diagnostics.is_empty());
    assert!(result.initial_diagnostics.iter().all(|d| d.fixable));
    assert_eq!(result.replacements.len(), 1);
    let edits = &result.replacements[0];
    assert_eq!(edits.path, "src/lib.rs");
    assert_eq!(edits.edits.len(), 1);
    assert_eq!(edits.edits[0].start_byte, 0);
    assert_eq!(edits.edits[0].end_byte, 6);
    assert_eq!(edits.edits[0].replacement, b"GOOD GOO");
    assert!(validate(&result).is_ok());
    let first = encode_validated(&result).unwrap();
    let rerun = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(encode_validated(&rerun).unwrap(), first);
    assert_eq!(decode_validated(&first).unwrap(), result);
}

#[test]
fn clean_inputs_converge_in_one_round_without_replacements() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "GOOD\n")];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 1);
    assert!(result.initial_diagnostics.is_empty());
    assert!(result.terminal_diagnostics.is_empty());
    assert!(result.replacements.is_empty());
    assert_eq!(
        result.original_snapshot[0].digest,
        result.terminal_snapshot[0].digest
    );
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn format_trims_trailing_whitespace_idempotently() {
    let stages = vec![stage("fmt-a", &["python"], &["src/main.py"])];
    let files = vec![file("src/main.py", "x  \ny\t\nz\n")];
    let result = run_pipeline("//quality:test", "format", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert!(result.initial_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].edits[0].start_byte, 1);
    assert_eq!(result.replacements[0].edits[0].end_byte, 6);
    assert_eq!(result.replacements[0].edits[0].replacement, b"\ny");
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn format_trims_final_line_without_trailing_newline() {
    let stages = vec![stage("fmt-a", &["python"], &["src/main.py"])];
    let files = vec![file("src/main.py", "x  \ny  ")];
    let result = run_pipeline("//quality:test", "format", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].edits[0].start_byte, 1);
    assert_eq!(result.replacements[0].edits[0].end_byte, 7);
    assert_eq!(result.replacements[0].edits[0].replacement, b"\ny");
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn diagnostic_only_tool_reports_without_edits() {
    let stages = vec![stage("lint-b", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "ok FAIL end\n")];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 1);
    assert_eq!(result.initial_diagnostics.len(), 1);
    let diagnostic = &result.initial_diagnostics[0];
    assert_eq!(diagnostic.severity, Severity::Error as i32);
    assert_eq!(diagnostic.tool_id, "lint-b");
    assert_eq!(diagnostic.start_byte, Some(3));
    assert_eq!(diagnostic.end_byte, Some(7));
    assert!(!diagnostic.fixable);
    assert_eq!(result.terminal_diagnostics.len(), 1);
    assert!(result.replacements.is_empty());
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn mixed_pipeline_marks_only_resolved_findings_fixable() {
    let stages = vec![
        stage("lint-a", &["rust"], &["src/a.rs"]),
        stage("lint-b", &["rust"], &["src/b.rs"]),
    ];
    let files = vec![file("src/a.rs", "BAD\n"), file("src/b.rs", "FAIL\n")];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.initial_diagnostics.len(), 2);
    assert_eq!(result.initial_diagnostics[0].path, "src/a.rs");
    assert_eq!(result.initial_diagnostics[1].path, "src/b.rs");
    assert!(result.initial_diagnostics[0].fixable);
    assert!(!result.initial_diagnostics[1].fixable);
    assert_eq!(result.terminal_diagnostics.len(), 1);
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].path, "src/a.rs");
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn all_capabilities_run() {
    let stages = vec![stage("lint-b", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "ok\n")];
    for (capability, expected) in [
        ("lint", Capability::Lint as i32),
        ("typecheck", Capability::Typecheck as i32),
        ("format", Capability::Format as i32),
        ("audit", Capability::Audit as i32),
    ] {
        let result = run_pipeline("//quality:test", capability, &stages, &files).unwrap();
        assert_eq!(result.capability, expected);
        assert!(encode_validated(&result).is_ok());
    }
}

#[test]
fn empty_file_has_no_findings() {
    let stages = vec![stage("fmt-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "")];
    let result = run_pipeline("//quality:test", "format", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert!(result.initial_diagnostics.is_empty());
    assert!(result.replacements.is_empty());
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn empty_producer_fails() {
    let (stages, files) = lint_fix();
    assert_eq!(
        run_pipeline("", "lint", &stages, &files),
        Err(RunnerError::EmptyProducer)
    );
}

#[test]
fn unknown_capability_fails() {
    let (stages, files) = lint_fix();
    assert_eq!(
        run_pipeline("//quality:test", "smell", &stages, &files),
        Err(RunnerError::UnknownCapability {
            capability: "smell".to_owned(),
        })
    );
}

#[test]
fn empty_stages_fails() {
    let (_, files) = lint_fix();
    assert_eq!(
        run_pipeline("//quality:test", "lint", &[], &files),
        Err(RunnerError::EmptyStages)
    );
}

#[test]
fn empty_tool_id_fails() {
    let (_, files) = lint_fix();
    let stages = vec![stage("", &["rust"], &["src/lib.rs"])];
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::EmptyToolId { stage: 0 })
    );
}

#[test]
fn unknown_tool_fails() {
    let (_, files) = lint_fix();
    let stages = vec![stage("nope", &["rust"], &["src/lib.rs"])];
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::UnknownTool {
            tool_id: "nope".to_owned(),
        })
    );
}

#[test]
fn empty_class_ids_fails() {
    let (_, files) = lint_fix();
    let stages = vec![stage("lint-a", &[], &["src/lib.rs"])];
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::EmptyClassIds { stage: 0 })
    );
}

#[test]
fn empty_stage_sources_fails() {
    let (_, files) = lint_fix();
    let stages = vec![stage("lint-a", &["rust"], &[])];
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::EmptyStageSources { stage: 0 })
    );
}

#[test]
fn duplicate_file_fails() {
    let (stages, _) = lint_fix();
    let files = vec![file("src/lib.rs", "BAD\n"), file("src/lib.rs", "BAD\n")];
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::DuplicateFile {
            path: "src/lib.rs".to_owned(),
        })
    );
}

#[test]
fn invalid_utf8_fails() {
    let (stages, _) = lint_fix();
    let files = vec![FileInput {
        path: "src/lib.rs".to_owned(),
        bytes: vec![0xFF],
    }];
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::InvalidUtf8 {
            path: "src/lib.rs".to_owned(),
        })
    );
}

#[test]
fn missing_file_fails() {
    let stages = vec![stage("lint-a", &["rust"], &["src/missing.rs"])];
    let (_, files) = lint_fix();
    assert_eq!(
        run_pipeline("//quality:test", "lint", &stages, &files),
        Err(RunnerError::MissingFile {
            path: "src/missing.rs".to_owned(),
        })
    );
}

#[test]
fn oscillation_is_detected() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let flip = |_: &str, _: &str, text: &str| {
        if text == "a" {
            Ok("b".to_owned())
        } else {
            Ok("a".to_owned())
        }
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, 10, flip).expect("converged");
    assert_eq!(convergence, Convergence::Oscillation);
    assert_eq!(completed, 2);
    assert_eq!(terminal["src/lib.rs"], "a");
}

#[test]
fn iteration_limit_is_detected() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let grow = |_: &str, _: &str, text: &str| Ok(format!("{text}x"));
    let (_, completed, convergence) =
        run_convergence(&initial, &stages, 3, grow).expect("converged");
    assert_eq!(convergence, Convergence::IterationLimit);
    assert_eq!(completed, 3);
}

#[test]
fn convergence_scales_linearly_in_rounds_and_files() {
    const FILES: usize = 300;
    const ROUNDS: u32 = 4_000;
    let paths: Vec<String> = (0..FILES).map(|i| format!("src/f{i:03}.rs")).collect();
    let last = paths.last().expect("files").clone();
    let stages = vec![StageSpec {
        tool_id: "lint-a".to_owned(),
        class_ids: vec!["rust".to_owned()],
        source_paths: paths.clone(),
    }];
    let mut initial = BTreeMap::new();
    for path in &paths {
        initial.insert(path.clone(), "v0".to_owned());
    }
    let counter = std::cell::Cell::new(0u32);
    let bump_last = |_: &str, path: &str, text: &str| {
        if path == last {
            let n = counter.get() + 1;
            counter.set(n);
            Ok(format!("{text}+{n}"))
        } else {
            Ok(text.to_owned())
        }
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, ROUNDS, bump_last).expect("converged");
    assert_eq!(convergence, Convergence::IterationLimit);
    assert_eq!(completed, ROUNDS);
    assert_eq!(counter.get(), ROUNDS);
    assert_eq!(
        terminal.get(&last).expect("last file staged"),
        &format!(
            "v0{}",
            (1..=ROUNDS).map(|n| format!("+{n}")).collect::<String>()
        )
    );
}

#[test]
fn unchanged_core_state_is_stable() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let same = |_: &str, _: &str, text: &str| Ok(text.to_owned());
    let (_, completed, convergence) =
        run_convergence(&initial, &stages, 10, same).expect("converged");
    assert_eq!(convergence, Convergence::Stable);
    assert_eq!(completed, 1);
}

#[test]
fn apply_failure_aborts_convergence() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let fail = |_: &str, _: &str, _: &str| Err(RunnerError::EmptyProducer);
    assert_eq!(
        run_convergence(&initial, &stages, 10, fail),
        Err(RunnerError::EmptyProducer)
    );
}

#[test]
fn synthetic_fallback_is_identity() {
    assert_eq!(apply_synthetic("unknown-tool", "x"), "x");
}

#[test]
fn non_diagnosing_tools_collect_no_diagnostics() {
    assert!(collect_diagnostics("fmt-a", "src/lib.rs", "BAD").is_empty());
    assert!(collect_diagnostics("unknown-tool", "src/lib.rs", "BAD").is_empty());
}

#[test]
fn error_display_reports_variant() {
    let rendered = format!(
        "{}",
        RunnerError::MissingFile {
            path: "src/lib.rs".to_owned(),
        }
    );
    assert!(rendered.contains("missing file"));
}

#[test]
fn convergence_reports_missing_stage_path_without_panicking() {
    let stages = vec![stage("lint-a", &["rust"], &["src/missing.rs"])];
    let initial = BTreeMap::new();
    let same = |_: &str, _: &str, text: &str| Ok(text.to_owned());
    assert_eq!(
        run_convergence(&initial, &stages, 10, same),
        Err(RunnerError::MissingFile {
            path: "src/missing.rs".to_owned(),
        })
    );
}

#[test]
fn stage_subset_reports_missing_path_without_panicking() {
    let stages = vec![stage("lint-a", &["rust"], &["src/missing.rs"])];
    let files = BTreeMap::new();
    assert_eq!(
        stage_subset(&stages[0], &files),
        Err(RunnerError::MissingFile {
            path: "src/missing.rs".to_owned(),
        })
    );
}

#[test]
fn diagnostics_sort_deterministically_under_shuffled_arrival() {
    fn diag(path: &str, start: u64, tool: &str, message: &str) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning as i32,
            message: message.to_owned(),
            tool_id: tool.to_owned(),
            path: path.to_owned(),
            start_byte: Some(start),
            end_byte: Some(start + 1),
            fixable: false,
            ..Default::default()
        }
    }
    let canonical = vec![
        diag("src/a.rs", 0, "lint-a", "first"),
        diag("src/a.rs", 5, "lint-a", "second"),
        diag("src/b.rs", 0, "lint-b", "third"),
    ];
    let mut reversed = canonical.clone();
    reversed.reverse();
    sort_diagnostics(&mut reversed);
    assert_eq!(reversed, canonical);
    let mut rotated = canonical.clone();
    rotated.rotate_left(1);
    sort_diagnostics(&mut rotated);
    assert_eq!(rotated, canonical);
}

#[test]
fn state_digest_independent_of_insertion_order() {
    let mut forward = BTreeMap::new();
    forward.insert("src/a.rs".to_owned(), "GOOD\n".to_owned());
    forward.insert("src/b.rs".to_owned(), "GOOD GOOD\n".to_owned());
    forward.insert("src/c.rs".to_owned(), "".to_owned());
    let mut backward = BTreeMap::new();
    backward.insert("src/c.rs".to_owned(), "".to_owned());
    backward.insert("src/b.rs".to_owned(), "GOOD GOOD\n".to_owned());
    backward.insert("src/a.rs".to_owned(), "GOOD\n".to_owned());
    assert_eq!(state_digest(&forward), state_digest(&backward));
    let mut mutated = forward.clone();
    mutated.insert("src/b.rs".to_owned(), "GOOD BAD\n".to_owned());
    assert_ne!(state_digest(&forward), state_digest(&mutated));
}

#[test]
fn diagnostics_tiebreak_deterministically_across_tool_and_message() {
    fn diag(tool: &str, message: &str) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning as i32,
            message: message.to_owned(),
            tool_id: tool.to_owned(),
            path: "src/same.rs".to_owned(),
            start_byte: Some(3),
            end_byte: Some(4),
            fixable: false,
            ..Default::default()
        }
    }
    let canonical = vec![
        diag("lint-a", "alpha"),
        diag("lint-a", "beta"),
        diag("lint-b", "alpha"),
    ];
    let mut reversed = canonical.clone();
    reversed.reverse();
    sort_diagnostics(&mut reversed);
    assert_eq!(reversed, canonical);
    let mut rotated = canonical.clone();
    rotated.rotate_left(2);
    sort_diagnostics(&mut rotated);
    assert_eq!(rotated, canonical);
}

#[test]
fn assemble_emits_replacements_only_when_stable() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "BAD\n".to_owned());
    let mut terminal = BTreeMap::new();
    terminal.insert("src/lib.rs".to_owned(), "GOOD\n".to_owned());

    let stable = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &terminal,
        (Vec::new(), Vec::new()),
        (2, Convergence::Stable),
    )
    .unwrap();
    assert_eq!(stable.replacements.len(), 1);
    let edits = &stable.replacements[0];
    assert_eq!(edits.path, "src/lib.rs");
    assert_eq!(edits.original_digest, digest("BAD\n".as_bytes()));
    assert_eq!(edits.edits.len(), 1);
    assert_eq!(edits.edits[0].start_byte, 0);
    assert_eq!(edits.edits[0].end_byte, 2);
    let original = "BAD\n";
    let applied = format!(
        "{}{}{}",
        &original[..edits.edits[0].start_byte as usize],
        String::from_utf8_lossy(&edits.edits[0].replacement),
        &original[edits.edits[0].end_byte as usize..]
    );
    assert_eq!(applied, "GOOD\n");

    let unchanged = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &initial,
        (Vec::new(), Vec::new()),
        (1, Convergence::Stable),
    )
    .unwrap();
    assert!(unchanged.replacements.is_empty());

    let oscillating = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &terminal,
        (Vec::new(), Vec::new()),
        (10, Convergence::Oscillation),
    )
    .unwrap();
    assert!(oscillating.replacements.is_empty());
}

#[test]
fn changing_tenth_round_fails_without_eleventh_invocation() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let invocations = std::cell::Cell::new(0u32);
    let grow = |_: &str, _: &str, text: &str| {
        invocations.set(invocations.get() + 1);
        Ok(format!("{text}x"))
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, grow).expect("converged");
    assert_eq!(convergence, Convergence::IterationLimit);
    assert_eq!(completed, MAX_COMPLETED_ROUNDS);
    assert_eq!(invocations.get(), MAX_COMPLETED_ROUNDS);
    assert_ne!(terminal["src/lib.rs"], initial["src/lib.rs"]);

    let result = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &terminal,
        (Vec::new(), Vec::new()),
        (completed, convergence),
    )
    .unwrap();
    assert!(result.replacements.is_empty());
    assert!(quality_result::validate(&result).is_ok());
}

#[test]
fn full_round_revert_reports_oscillation_not_stability() {
    let stages = vec![
        stage("lint-a", &["rust"], &["src/lib.rs"]),
        stage("lint-b", &["rust"], &["src/lib.rs"]),
    ];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let undo = |tool: &str, _: &str, text: &str| match (tool, text) {
        ("lint-a", "a") => Ok("b".to_owned()),
        ("lint-b", "b") => Ok("a".to_owned()),
        _ => Ok(text.to_owned()),
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, 10, undo).expect("converged");
    assert_eq!(convergence, Convergence::Oscillation);
    assert_eq!(completed, 1);
    assert_eq!(terminal["src/lib.rs"], "a");
    let result = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &terminal,
        (Vec::new(), Vec::new()),
        (completed, convergence),
    )
    .unwrap();
    assert!(result.replacements.is_empty());
    assert!(validate(&result).is_ok());
}

#[test]
fn diagnostics_sort_includes_severity_and_rule() {
    fn diag(severity: i32, tool: &str, rule: &str, message: &str) -> Diagnostic {
        Diagnostic {
            severity,
            message: message.to_owned(),
            tool_id: tool.to_owned(),
            rule_id: rule.to_owned(),
            path: "src/same.rs".to_owned(),
            start_byte: Some(3),
            end_byte: Some(4),
            fixable: false,
            ..Default::default()
        }
    }
    let canonical = vec![
        diag(Severity::Info as i32, "lint-a", "a-rule", "alpha"),
        diag(Severity::Warning as i32, "lint-a", "a-rule", "alpha"),
        diag(Severity::Warning as i32, "lint-a", "b-rule", "alpha"),
        diag(Severity::Warning as i32, "lint-b", "a-rule", "alpha"),
        diag(Severity::Error as i32, "lint-a", "a-rule", "alpha"),
    ];
    let mut reversed = canonical.clone();
    reversed.reverse();
    sort_diagnostics(&mut reversed);
    assert_eq!(reversed, canonical);
    let mut rotated = canonical.clone();
    rotated.rotate_left(2);
    sort_diagnostics(&mut rotated);
    assert_eq!(rotated, canonical);
}

#[test]
fn assemble_replacements_follow_sorted_path_order_for_atomic_apply() {
    let stages = vec![stage(
        "lint-a",
        &["rust"],
        &["src/c.rs", "src/a.rs", "src/b.rs"],
    )];
    let mut initial = BTreeMap::new();
    initial.insert("src/c.rs".to_owned(), "BAD c\n".to_owned());
    initial.insert("src/b.rs".to_owned(), "BAD b\n".to_owned());
    initial.insert("src/a.rs".to_owned(), "BAD a\n".to_owned());
    let mut terminal = BTreeMap::new();
    terminal.insert("src/c.rs".to_owned(), "GOOD c\n".to_owned());
    terminal.insert("src/b.rs".to_owned(), "GOOD b\n".to_owned());
    terminal.insert("src/a.rs".to_owned(), "GOOD a\n".to_owned());

    let result = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &terminal,
        (Vec::new(), Vec::new()),
        (2, Convergence::Stable),
    )
    .unwrap();
    assert_eq!(result.replacements.len(), 3);
    let paths: Vec<&str> = result
        .replacements
        .iter()
        .map(|edits| edits.path.as_str())
        .collect();
    assert_eq!(paths, vec!["src/a.rs", "src/b.rs", "src/c.rs"]);
    for edits in &result.replacements {
        let original = initial.get(&edits.path).expect("staged path");
        let terminal_body = terminal.get(&edits.path).expect("staged path");
        assert_eq!(edits.original_digest, digest(original.as_bytes()));
        assert_eq!(edits.edits.len(), 1);
        let edit = &edits.edits[0];
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&original.as_bytes()[..edit.start_byte as usize]);
        spliced.extend_from_slice(&edit.replacement);
        spliced.extend_from_slice(&original.as_bytes()[edit.end_byte as usize..]);
        assert_eq!(&spliced, terminal_body.as_bytes());
    }
    let prefix: Vec<&str> = paths.iter().take(2).copied().collect();
    assert_eq!(prefix, vec!["src/a.rs", "src/b.rs"]);
    assert!(quality_result::validate(&result).is_ok());
}
