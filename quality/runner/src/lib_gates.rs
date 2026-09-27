use super::*;
use quality_result::{assert_all_equal, decode_validated, encode_validated, validate};

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

#[test]
fn diagnostic_envelope_rejected_by_validate_gate() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "BAD\n")];
    let valid = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert!(!valid.initial_diagnostics.is_empty());
    assert!(validate(&valid).is_ok());
    let mut bad_severity = valid.clone();
    bad_severity.initial_diagnostics[0].severity = Severity::Unspecified as i32;
    assert!(validate(&bad_severity).is_err());
    let mut empty_message = valid.clone();
    empty_message.initial_diagnostics[0].message = String::new();
    assert!(validate(&empty_message).is_err());
    let mut empty_tool = valid.clone();
    empty_tool.initial_diagnostics[0].tool_id = String::new();
    assert!(validate(&empty_tool).is_err());
    let mut range_without_path = valid.clone();
    range_without_path.initial_diagnostics[0].path = String::new();
    assert!(validate(&range_without_path).is_err());
    let mut missing_range = valid.clone();
    missing_range.initial_diagnostics[0].start_byte = None;
    assert!(validate(&missing_range).is_err());
    let mut inverted_range = valid.clone();
    inverted_range.initial_diagnostics[0].start_byte = Some(3);
    inverted_range.initial_diagnostics[0].end_byte = Some(2);
    assert!(validate(&inverted_range).is_err());
}

#[test]
fn unstable_envelope_with_replacements_rejected_by_validate_gate() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "BAD\n")];
    let valid = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(valid.convergence, Convergence::Stable as i32);
    assert!(!valid.replacements.is_empty());
    assert!(validate(&valid).is_ok());
    let mut limited = valid.clone();
    limited.convergence = Convergence::IterationLimit as i32;
    assert!(validate(&limited).is_err());
    let mut oscillating = valid.clone();
    oscillating.convergence = Convergence::Oscillation as i32;
    assert!(validate(&oscillating).is_err());
    let mut unstable_empty = valid.clone();
    unstable_empty.convergence = Convergence::IterationLimit as i32;
    unstable_empty.replacements = Vec::new();
    assert!(validate(&unstable_empty).is_ok());
}

#[test]
fn newline_variants_yield_distinct_manifests() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let variants = ["BAD\n", "BAD", "BAD\r\n"];
    let terminals = ["GOOD\n", "GOOD", "GOOD\r\n"];
    let mut manifests = Vec::with_capacity(variants.len());
    for (index, body) in variants.iter().enumerate() {
        let files = vec![file("src/lib.rs", body)];
        let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
        assert_eq!(result.convergence, Convergence::Stable as i32);
        assert_eq!(result.replacements.len(), 1);
        assert_eq!(
            result.replacements[0].original_digest,
            digest(body.as_bytes())
        );
        assert_eq!(result.replacements[0].edits.len(), 1);
        let edit = &result.replacements[0].edits[0];
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&body.as_bytes()[..edit.start_byte as usize]);
        spliced.extend_from_slice(&edit.replacement);
        spliced.extend_from_slice(&body.as_bytes()[edit.end_byte as usize..]);
        assert_eq!(&spliced, terminals[index].as_bytes());
        assert!(validate(&result).is_ok());
        manifests.push(encode_validated(&result).unwrap());
    }
    assert_ne!(manifests[0], manifests[1]);
    assert_ne!(manifests[0], manifests[2]);
    assert_ne!(manifests[1], manifests[2]);
    let repeat = vec![file("src/lib.rs", "BAD\n")];
    let rerun = run_pipeline("//quality:test", "lint", &stages, &repeat).unwrap();
    assert_eq!(manifests[0], encode_validated(&rerun).unwrap());
}

#[test]
fn quality_originated_file_creates_emit_no_replacements() {
    let stages = vec![stage("lint-a", &["rust"], &["src/a.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/a.rs".to_owned(), "BAD\n".to_owned());
    let mut terminal = BTreeMap::new();
    terminal.insert("src/a.rs".to_owned(), "GOOD\n".to_owned());
    terminal.insert("src/extra.rs".to_owned(), "GOOD extra\n".to_owned());
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
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].path, "src/a.rs");
    assert_eq!(
        result.replacements[0].original_digest,
        digest("BAD\n".as_bytes())
    );
    assert_eq!(result.replacements[0].edits.len(), 1);
    assert_eq!(result.replacements[0].edits[0].start_byte, 0);
    assert_eq!(result.replacements[0].edits[0].end_byte, 2);
    assert_eq!(result.replacements[0].edits[0].replacement, b"GOO".to_vec());
    assert!(validate(&result).is_ok());
}

#[test]
fn file_modes_do_not_alter_pipeline_outputs() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let modes = [0o644, 0o755, 0o600];
    let mut manifests = Vec::with_capacity(modes.len());
    for mode in modes {
        let _ = mode;
        let files = vec![file("src/lib.rs", "BAD\n")];
        let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
        assert_eq!(result.convergence, Convergence::Stable as i32);
        assert_eq!(result.replacements.len(), 1);
        assert_eq!(
            result.replacements[0].original_digest,
            digest("BAD\n".as_bytes())
        );
        assert!(validate(&result).is_ok());
        manifests.push(encode_validated(&result).unwrap());
    }
    assert_all_equal(&manifests);
    let clean = vec![file("src/lib.rs", "GOOD\n")];
    let clean_result = run_pipeline("//quality:test", "lint", &stages, &clean).unwrap();
    assert!(clean_result.replacements.is_empty());
    assert_ne!(manifests[0], encode_validated(&clean_result).unwrap());
}

#[test]
fn invalid_utf8_replacement_rejected_by_validate_gate() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "BAD\n")];
    let valid = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(valid.replacements.len(), 1);
    assert!(validate(&valid).is_ok());
    let mut corrupted = valid.clone();
    corrupted.replacements[0].edits[0].replacement = vec![0xFF, 0xFE];
    assert!(validate(&corrupted).is_err());
    let bad_source = vec![FileInput {
        path: "src/lib.rs".to_owned(),
        bytes: vec![0xFF],
    }];
    assert!(run_pipeline("//quality:test", "lint", &stages, &bad_source).is_err());
}

#[test]
fn permutation_ranking_prefers_stable_fewer_rounds() {
    let forward = vec![
        stage("lint-a", &["rust"], &["src/lib.rs"]),
        stage("fmt-a", &["rust"], &["src/lib.rs"]),
    ];
    let reversed = vec![
        stage("fmt-a", &["rust"], &["src/lib.rs"]),
        stage("lint-a", &["rust"], &["src/lib.rs"]),
    ];
    let files = vec![file("src/lib.rs", "BAD   \n")];
    let first = run_pipeline("//quality:test", "lint", &forward, &files).unwrap();
    let second = run_pipeline("//quality:test", "lint", &reversed, &files).unwrap();
    assert_eq!(first.convergence, Convergence::Stable as i32);
    assert_eq!(second.convergence, Convergence::Stable as i32);
    assert_eq!(first.completed_rounds, second.completed_rounds);
    assert_eq!(first.terminal_snapshot, second.terminal_snapshot);
    assert_eq!(first.replacements, second.replacements);
    assert!(validate(&first).is_ok());
    assert!(validate(&second).is_ok());
    let rank_key = |result: &QualityResult| {
        let non_converged = i32::from(result.convergence != Convergence::Stable as i32);
        (non_converged, result.completed_rounds)
    };
    assert_eq!(rank_key(&first), rank_key(&second));
    assert_eq!(rank_key(&first), (0, 2));
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "BAD".to_owned());
    let direct = |_: &str, _: &str, text: &str| {
        if text == "BAD" {
            Ok("GOOD".to_owned())
        } else {
            Ok(text.to_owned())
        }
    };
    let gradual = |_: &str, _: &str, text: &str| {
        if text == "BAD" {
            Ok("MID".to_owned())
        } else if text == "MID" {
            Ok("GOOD".to_owned())
        } else {
            Ok(text.to_owned())
        }
    };
    let (fast_terminal, fast_rounds, fast_conv) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, direct).unwrap();
    let (slow_terminal, slow_rounds, slow_conv) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, gradual).unwrap();
    assert_eq!(fast_conv, Convergence::Stable);
    assert_eq!(slow_conv, Convergence::Stable);
    assert_eq!(fast_terminal["src/lib.rs"], "GOOD");
    assert_eq!(slow_terminal["src/lib.rs"], "GOOD");
    assert_eq!(fast_rounds, 2);
    assert_eq!(slow_rounds, 3);
    assert!(fast_rounds < slow_rounds);
    let flip = |_: &str, _: &str, text: &str| {
        if text == "a" {
            Ok("b".to_owned())
        } else {
            Ok("a".to_owned())
        }
    };
    let mut flip_initial = BTreeMap::new();
    flip_initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let (_, _, flip_conv) =
        run_convergence(&flip_initial, &stages, MAX_COMPLETED_ROUNDS, flip).unwrap();
    assert_eq!(flip_conv, Convergence::Oscillation);
    let grow = |_: &str, _: &str, text: &str| Ok(format!("{text}x"));
    let (_, _, grow_conv) = run_convergence(&flip_initial, &stages, 3, grow).unwrap();
    assert_eq!(grow_conv, Convergence::IterationLimit);
    let stable_rank = (0, fast_rounds);
    let oscillation_rank = (1, 2);
    let limit_rank = (1, 3);
    assert!(stable_rank < oscillation_rank);
    assert!(stable_rank < limit_rank);
    let fix_stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let keep_stages = vec![stage("lint-b", &["rust"], &["src/lib.rs"])];
    let bad_files = vec![file("src/lib.rs", "BAD\n")];
    let fixed = run_pipeline("//quality:test", "lint", &fix_stages, &bad_files).unwrap();
    let kept = run_pipeline("//quality:test", "lint", &keep_stages, &bad_files).unwrap();
    assert_ne!(fixed.terminal_snapshot, kept.terminal_snapshot);
}

#[test]
fn interruption_leaves_no_partially_written_file() {
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
    for edits in &result.replacements {
        assert_eq!(edits.edits.len(), 1);
        let original = initial.get(&edits.path).expect("staged path");
        let terminal_body = terminal.get(&edits.path).expect("staged path");
        let edit = &edits.edits[0];
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&original.as_bytes()[..edit.start_byte as usize]);
        spliced.extend_from_slice(&edit.replacement);
        spliced.extend_from_slice(&original.as_bytes()[edit.end_byte as usize..]);
        assert_eq!(&spliced, terminal_body.as_bytes());
    }
    let paths: Vec<&str> = result
        .replacements
        .iter()
        .map(|edits| edits.path.as_str())
        .collect();
    assert_eq!(paths, vec!["src/a.rs", "src/b.rs", "src/c.rs"]);
    for prefix_len in 0..=result.replacements.len() {
        let mut state = initial.clone();
        for edits in result.replacements.iter().take(prefix_len) {
            let original = initial.get(&edits.path).expect("staged path");
            let edit = &edits.edits[0];
            let mut spliced = Vec::new();
            spliced.extend_from_slice(&original.as_bytes()[..edit.start_byte as usize]);
            spliced.extend_from_slice(&edit.replacement);
            spliced.extend_from_slice(&original.as_bytes()[edit.end_byte as usize..]);
            state.insert(edits.path.clone(), String::from_utf8(spliced).unwrap());
        }
        for (path, body) in &state {
            let original = initial.get(path).expect("staged path");
            let terminal_body = terminal.get(path).expect("staged path");
            assert!(body == original || body == terminal_body);
        }
    }
    let mut full = initial.clone();
    for edits in &result.replacements {
        let original = initial.get(&edits.path).expect("staged path");
        let edit = &edits.edits[0];
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&original.as_bytes()[..edit.start_byte as usize]);
        spliced.extend_from_slice(&edit.replacement);
        spliced.extend_from_slice(&original.as_bytes()[edit.end_byte as usize..]);
        full.insert(edits.path.clone(), String::from_utf8(spliced).unwrap());
    }
    assert_eq!(full, terminal);
    assert!(validate(&result).is_ok());
}

#[test]
fn checkout_and_query_permutations_converge_identically() {
    let checkouts = ["/tmp/checkout-a", "/home/user/work/tree"];
    let mut manifests = Vec::with_capacity(checkouts.len());
    for (index, prefix) in checkouts.iter().enumerate() {
        let _ = prefix;
        let stages = if index == 0 {
            vec![stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"])]
        } else {
            vec![stage("lint-a", &["rust"], &["src/b.rs", "src/a.rs"])]
        };
        let files = if index == 0 {
            vec![file("src/a.rs", "BAD a\n"), file("src/b.rs", "BAD b\n")]
        } else {
            vec![file("src/b.rs", "BAD b\n"), file("src/a.rs", "BAD a\n")]
        };
        let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
        assert_eq!(result.convergence, Convergence::Stable as i32);
        assert_eq!(result.completed_rounds, 2);
        assert_eq!(result.replacements.len(), 2);
        assert!(validate(&result).is_ok());
        manifests.push(encode_validated(&result).unwrap());
        assert!(format!("{prefix}/src/a.rs").ends_with("src/a.rs"));
    }
    let decoded: Vec<QualityResult> = manifests
        .iter()
        .map(|bytes| decode_validated(bytes).expect("decode"))
        .collect();
    assert_eq!(decoded[0].original_snapshot, decoded[1].original_snapshot);
    assert_eq!(decoded[0].terminal_snapshot, decoded[1].terminal_snapshot);
    assert_eq!(
        decoded[0].initial_diagnostics,
        decoded[1].initial_diagnostics
    );
    assert_eq!(
        decoded[0].terminal_diagnostics,
        decoded[1].terminal_diagnostics
    );
    assert_eq!(decoded[0].replacements, decoded[1].replacements);
    assert_eq!(decoded[0].completed_rounds, decoded[1].completed_rounds);
    let mut first_sources = decoded[0].stages[0].source_paths.clone();
    let mut second_sources = decoded[1].stages[0].source_paths.clone();
    first_sources.sort();
    second_sources.sort();
    assert_eq!(first_sources, second_sources);
    let stages = vec![stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"])];
    let forward = vec![file("src/a.rs", "BAD a\n"), file("src/b.rs", "BAD b\n")];
    let reversed = vec![file("src/b.rs", "BAD b\n"), file("src/a.rs", "BAD a\n")];
    let first = run_pipeline("//quality:test", "lint", &stages, &forward).unwrap();
    let second = run_pipeline("//quality:test", "lint", &stages, &reversed).unwrap();
    assert_eq!(first.replacements, second.replacements);
    assert_eq!(manifests[0], encode_validated(&first).unwrap());
}

#[test]
fn permutation_ranking_uses_process_starts_then_wall_time_tiebreak() {
    let single = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let doubled = vec![
        stage("lint-a", &["rust"], &["src/lib.rs"]),
        stage("lint-b", &["rust"], &["src/lib.rs"]),
    ];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "BAD\n".to_owned());
    let single_calls = std::cell::RefCell::new(0u32);
    let single_counting = |tool: &str, _: &str, text: &str| {
        *single_calls.borrow_mut() += 1;
        Ok(apply_synthetic(tool, text))
    };
    let (single_terminal, single_rounds, single_conv) =
        run_convergence(&initial, &single, MAX_COMPLETED_ROUNDS, single_counting).unwrap();
    let doubled_calls = std::cell::RefCell::new(0u32);
    let doubled_counting = |tool: &str, _: &str, text: &str| {
        *doubled_calls.borrow_mut() += 1;
        Ok(apply_synthetic(tool, text))
    };
    let (doubled_terminal, doubled_rounds, doubled_conv) =
        run_convergence(&initial, &doubled, MAX_COMPLETED_ROUNDS, doubled_counting).unwrap();
    assert_eq!(single_conv, Convergence::Stable);
    assert_eq!(doubled_conv, Convergence::Stable);
    assert_eq!(single_terminal["src/lib.rs"], "GOOD\n");
    assert_eq!(doubled_terminal["src/lib.rs"], "GOOD\n");
    assert_eq!(single_terminal, doubled_terminal);
    assert_eq!(single_rounds, 2);
    assert_eq!(doubled_rounds, 2);
    let single_starts = *single_calls.borrow();
    let doubled_starts = *doubled_calls.borrow();
    assert_eq!(single_starts, 2);
    assert_eq!(doubled_starts, 4);
    let rank_key = |convergence: Convergence, rounds: u32, starts: u32, wall_ms: u64| {
        (
            i32::from(convergence != Convergence::Stable),
            rounds,
            starts,
            wall_ms,
        )
    };
    let single_rank = rank_key(Convergence::Stable, single_rounds, single_starts, 10);
    let doubled_rank = rank_key(Convergence::Stable, doubled_rounds, doubled_starts, 5);
    assert!(
        single_rank < doubled_rank,
        "fewer process starts ranks first even with larger wall time"
    );
    let fast_rank = rank_key(Convergence::Stable, 2, 2, 10);
    let slow_rank = rank_key(Convergence::Stable, 2, 2, 20);
    assert!(
        fast_rank < slow_rank,
        "smaller wall time ranks first on full tie"
    );
    assert_eq!(fast_rank, (0, 2, 2, 10));
}

#[test]
fn each_stage_runs_once_per_round_without_hidden_passes() {
    let stages = vec![
        stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"]),
        stage("fmt-a", &["rust"], &["src/a.rs"]),
    ];
    let mut initial = BTreeMap::new();
    initial.insert("src/a.rs".to_owned(), "BAD   \n".to_owned());
    initial.insert("src/b.rs".to_owned(), "BAD\n".to_owned());
    let calls = std::cell::RefCell::new(BTreeMap::new());
    let counting = |tool: &str, path: &str, text: &str| {
        *calls
            .borrow_mut()
            .entry((tool.to_owned(), path.to_owned()))
            .or_insert(0u32) += 1;
        Ok(apply_synthetic(tool, text))
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, counting).unwrap();
    assert_eq!(convergence, Convergence::Stable);
    assert_eq!(completed, 2);
    assert_eq!(terminal["src/a.rs"], "GOOD\n");
    assert_eq!(terminal["src/b.rs"], "GOOD\n");
    let calls = calls.borrow();
    assert_eq!(calls.values().sum::<u32>(), 6);
    assert_eq!(calls.len(), 3);
    for pair in [
        ("lint-a", "src/a.rs"),
        ("lint-a", "src/b.rs"),
        ("fmt-a", "src/a.rs"),
    ] {
        assert_eq!(
            calls[&(pair.0.to_owned(), pair.1.to_owned())],
            2,
            "each staged (tool, path) runs once per round"
        );
    }
}

#[test]
fn minimal_edits_trim_common_prefix_and_suffix() {
    let cases = vec![
        ("BAD\n", "GOOD\n", 0, 2, "GOO"),
        ("BAD BAD\n", "GOOD GOOD\n", 0, 6, "GOOD GOO"),
        ("x  \n", "x\n", 1, 3, ""),
        ("", "GOOD\n", 0, 0, "GOOD\n"),
        ("BAD\n", "", 0, 4, ""),
    ];
    for (original, terminal_body, start, end, replacement) in cases {
        let edit = minimal_edit(original, terminal_body);
        assert_eq!(edit.start_byte, start, "{original:?}");
        assert_eq!(edit.end_byte, end, "{original:?}");
        assert_eq!(edit.replacement, replacement.as_bytes(), "{original:?}");
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&original.as_bytes()[..edit.start_byte as usize]);
        spliced.extend_from_slice(&edit.replacement);
        spliced.extend_from_slice(&original.as_bytes()[edit.end_byte as usize..]);
        assert_eq!(&spliced, terminal_body.as_bytes(), "{original:?}");
    }
    let edit = minimal_edit("héllo BAD 🌍\n", "héllo GOOD 🌍\n");
    assert!("héllo BAD 🌍\n".is_char_boundary(edit.start_byte as usize));
    assert!("héllo BAD 🌍\n".is_char_boundary(edit.end_byte as usize));
    let mut spliced = Vec::new();
    spliced.extend_from_slice(&"héllo BAD 🌍\n".as_bytes()[..edit.start_byte as usize]);
    spliced.extend_from_slice(&edit.replacement);
    spliced.extend_from_slice(&"héllo BAD 🌍\n".as_bytes()[edit.end_byte as usize..]);
    assert_eq!(&spliced, "héllo GOOD 🌍\n".as_bytes());
}

#[test]
fn per_capability_round_caps_short_circuit_check_only() {
    assert_eq!(max_rounds_for_capability("audit"), 1);
    assert_eq!(max_rounds_for_capability("typecheck"), 1);
    assert_eq!(max_rounds_for_capability("lint"), MAX_COMPLETED_ROUNDS);
    assert_eq!(max_rounds_for_capability("format"), MAX_COMPLETED_ROUNDS);
}

#[test]
fn unchanged_terminal_reuses_initial_diagnostics() {
    let stages = vec![stage("lint-b", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "ok FAIL end\n")];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 1);
    assert_eq!(result.initial_diagnostics, result.terminal_diagnostics);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert!(result.replacements.is_empty());
    assert!(encode_validated(&result).is_ok());
}
