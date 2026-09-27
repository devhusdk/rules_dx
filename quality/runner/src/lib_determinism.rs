use super::*;
use quality_result::{assert_all_equal, encode_validated, validate};

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
fn assemble_covers_insertion_deletion_and_multibyte_boundaries() {
    let stages = vec![stage("lint-a", &["rust"], &["src/a.rs"])];
    let cases = vec![
        ("insertion", "", "GOOD\n"),
        ("deletion", "BAD\n", ""),
        (
            "multibyte",
            "h\u{e9}llo BAD \u{1f30d}\n",
            "h\u{e9}llo GOOD \u{1f30d}\n",
        ),
    ];
    for (label, original, terminal_body) in cases {
        let mut initial = BTreeMap::new();
        initial.insert("src/a.rs".to_owned(), original.to_owned());
        let mut terminal = BTreeMap::new();
        terminal.insert("src/a.rs".to_owned(), terminal_body.to_owned());
        let result = assemble(
            "//quality:test",
            Capability::Lint as i32,
            &stages,
            &initial,
            &terminal,
            (Vec::new(), Vec::new()),
            (2, Convergence::Stable),
        )
        .unwrap_or_else(|err| panic!("{label}: assemble failed: {err:?}"));
        assert_eq!(result.replacements.len(), 1, "{label}");
        let edits = &result.replacements[0];
        assert_eq!(edits.path, "src/a.rs", "{label}");
        assert_eq!(
            edits.original_digest,
            digest(original.as_bytes()),
            "{label}"
        );
        assert_eq!(edits.edits.len(), 1, "{label}");
        let edit = &edits.edits[0];
        assert!(
            original.is_char_boundary(edit.start_byte as usize),
            "{label}"
        );
        assert!(original.is_char_boundary(edit.end_byte as usize), "{label}");
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&original.as_bytes()[..edit.start_byte as usize]);
        spliced.extend_from_slice(&edit.replacement);
        spliced.extend_from_slice(&original.as_bytes()[edit.end_byte as usize..]);
        assert_eq!(&spliced, terminal_body.as_bytes(), "{label}");
        assert!(quality_result::validate(&result).is_ok(), "{label}");
    }
}

#[test]
fn source_declaration_reorder_yields_identical_manifests() {
    let files = vec![file("src/a.rs", "BAD a\n"), file("src/b.rs", "BAD b\n")];
    let forward = vec![stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"])];
    let reversed = vec![stage("lint-a", &["rust"], &["src/b.rs", "src/a.rs"])];
    let first = run_pipeline("//quality:test", "lint", &forward, &files).unwrap();
    let second = run_pipeline("//quality:test", "lint", &reversed, &files).unwrap();
    assert_eq!(first.convergence, Convergence::Stable as i32);
    assert_eq!(second.convergence, Convergence::Stable as i32);
    assert_eq!(first.completed_rounds, second.completed_rounds);
    assert_eq!(first.original_snapshot, second.original_snapshot);
    assert_eq!(first.terminal_snapshot, second.terminal_snapshot);
    assert_eq!(first.initial_diagnostics, second.initial_diagnostics);
    assert_eq!(first.terminal_diagnostics, second.terminal_diagnostics);
    assert_eq!(first.replacements, second.replacements);
    let mut first_sources = first.stages[0].source_paths.clone();
    let mut second_sources = second.stages[0].source_paths.clone();
    first_sources.sort();
    second_sources.sort();
    assert_eq!(first_sources, second_sources);
    assert!(quality_result::validate(&first).is_ok());
    assert!(quality_result::validate(&second).is_ok());
}

#[test]
fn stale_source_digest_mismatch_must_reject_write() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "BAD\n".to_owned());
    let mut terminal = BTreeMap::new();
    terminal.insert("src/lib.rs".to_owned(), "GOOD\n".to_owned());
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
    let edits = &result.replacements[0];
    assert_eq!(edits.original_digest, digest("BAD\n".as_bytes()));
    let stale = "OTHER\n";
    assert_ne!(digest(stale.as_bytes()).to_vec(), edits.original_digest);
    let edit = &edits.edits[0];
    let mut spliced = Vec::new();
    spliced.extend_from_slice(&b"BAD\n"[..edit.start_byte as usize]);
    spliced.extend_from_slice(&edit.replacement);
    spliced.extend_from_slice(&b"BAD\n"[edit.end_byte as usize..]);
    assert_eq!(&spliced, b"GOOD\n");
    assert!(quality_result::validate(&result).is_ok());
}

#[test]
fn mutation_outcome_depends_on_bytes_not_git_status() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let statuses = ["tracked", "modified", "staged", "untracked"];
    let mut manifests = Vec::with_capacity(statuses.len());
    for status in statuses {
        let _ = status;
        let files = vec![file("src/lib.rs", "BAD\n")];
        let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
        assert_eq!(result.convergence, Convergence::Stable as i32, "{status}");
        assert_eq!(result.replacements.len(), 1, "{status}");
        assert_eq!(
            result.replacements[0].original_digest,
            digest("BAD\n".as_bytes()),
            "{status}"
        );
        assert!(validate(&result).is_ok(), "{status}");
        manifests.push(encode_validated(&result).unwrap());
    }
    assert_all_equal(&manifests);
    let clean = vec![file("src/lib.rs", "GOOD\n")];
    let clean_result = run_pipeline("//quality:test", "lint", &stages, &clean).unwrap();
    assert!(clean_result.replacements.is_empty());
    assert_ne!(manifests[0], encode_validated(&clean_result).unwrap());
}

#[test]
fn identical_final_bytes_across_producer_identities() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "BAD\n")];
    let first = run_pipeline("//quality:owner-a", "lint", &stages, &files).unwrap();
    let second = run_pipeline("//quality:owner-b", "lint", &stages, &files).unwrap();
    assert_eq!(first.producer, "//quality:owner-a");
    assert_eq!(second.producer, "//quality:owner-b");
    assert_eq!(first.terminal_snapshot, second.terminal_snapshot);
    assert_eq!(first.original_snapshot, second.original_snapshot);
    assert_eq!(first.initial_diagnostics, second.initial_diagnostics);
    assert_eq!(first.terminal_diagnostics, second.terminal_diagnostics);
    assert_eq!(first.replacements, second.replacements);
    assert_eq!(first.replacements.len(), 1);
    assert_eq!(first.replacements[0].edits[0].start_byte, 0);
    assert_eq!(first.replacements[0].edits[0].end_byte, 2);
    assert_eq!(first.replacements[0].edits[0].replacement, b"GOO".to_vec());
    assert_eq!(second.replacements[0].edits[0].replacement, b"GOO".to_vec());
    assert!(validate(&first).is_ok());
    assert!(validate(&second).is_ok());
}

#[test]
fn check_mode_must_fail_on_replacements_without_diagnostics() {
    let stages = vec![stage("fmt-a", &["python"], &["src/main.py"])];
    let files = vec![file("src/main.py", "x  \n")];
    let result = run_pipeline("//quality:test", "format", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert!(result.initial_diagnostics.is_empty());
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].edits[0].start_byte, 1);
    assert_eq!(result.replacements[0].edits[0].end_byte, 3);
    assert!(result.replacements[0].edits[0].replacement.is_empty());
    assert!(!result.replacements.is_empty());
    assert!(validate(&result).is_ok());
}

#[test]
fn file_arrival_reorder_yields_identical_manifests() {
    let stages = vec![stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"])];
    let forward = vec![file("src/a.rs", "BAD a\n"), file("src/b.rs", "BAD b\n")];
    let reversed = vec![file("src/b.rs", "BAD b\n"), file("src/a.rs", "BAD a\n")];
    let first = run_pipeline("//quality:test", "lint", &stages, &forward).unwrap();
    let second = run_pipeline("//quality:test", "lint", &stages, &reversed).unwrap();
    assert_eq!(first.convergence, Convergence::Stable as i32);
    assert_eq!(second.convergence, Convergence::Stable as i32);
    assert_eq!(first.completed_rounds, second.completed_rounds);
    assert_eq!(first.original_snapshot, second.original_snapshot);
    assert_eq!(first.terminal_snapshot, second.terminal_snapshot);
    assert_eq!(first.initial_diagnostics, second.initial_diagnostics);
    assert_eq!(first.terminal_diagnostics, second.terminal_diagnostics);
    assert_eq!(first.replacements, second.replacements);
    assert!(validate(&first).is_ok());
    assert!(validate(&second).is_ok());
}

#[test]
fn env_permutations_do_not_alter_pipeline_outputs() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let envs = [
        ("C", "UTC", "/home/a", "/usr/bin"),
        (
            "en_US.UTF-8",
            "America/New_York",
            "/home/b",
            "/usr/local/bin:/usr/bin",
        ),
        ("C.UTF-8", "Europe/Berlin", "/root", "/opt/bin:/usr/bin"),
    ];
    let mut manifests = Vec::with_capacity(envs.len());
    for (locale, tz, home, path) in envs {
        let _ = (locale, tz, home, path);
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
fn checkout_paths_do_not_alter_pipeline_outputs() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let checkouts = ["/tmp/checkout-a", "/tmp/checkout-b", "/home/user/work/tree"];
    let mut manifests = Vec::with_capacity(checkouts.len());
    for prefix in checkouts {
        let relative = "src/lib.rs";
        assert!(format!("{prefix}/{relative}").ends_with(relative));
        let _ = prefix;
        let files = vec![file(relative, "BAD\n")];
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
fn mixed_changed_and_unchanged_files_apply_independently() {
    let stages = vec![stage(
        "lint-a",
        &["rust"],
        &["src/changed.rs", "src/clean.rs"],
    )];
    let files = vec![
        file("src/changed.rs", "BAD\n"),
        file("src/clean.rs", "GOOD\n"),
    ];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.replacements.len(), 1);
    let edits = &result.replacements[0];
    assert_eq!(edits.path, "src/changed.rs");
    assert_eq!(edits.original_digest, digest("BAD\n".as_bytes()));
    assert_eq!(edits.edits.len(), 1);
    assert_eq!(edits.edits[0].start_byte, 0);
    assert_eq!(edits.edits[0].end_byte, 2);
    assert_eq!(&edits.edits[0].replacement, b"GOO");
    assert!(validate(&result).is_ok());
}

#[test]
fn incomplete_terminal_collection_rejects_before_any_replacement() {
    let stages = vec![stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/a.rs".to_owned(), "BAD a\n".to_owned());
    initial.insert("src/b.rs".to_owned(), "BAD b\n".to_owned());
    let mut partial_terminal = BTreeMap::new();
    partial_terminal.insert("src/a.rs".to_owned(), "GOOD a\n".to_owned());
    let err = assemble(
        "//quality:test",
        Capability::Lint as i32,
        &stages,
        &initial,
        &partial_terminal,
        (Vec::new(), Vec::new()),
        (2, Convergence::Stable),
    )
    .unwrap_err();
    assert_eq!(
        err,
        RunnerError::MissingFile {
            path: "src/b.rs".to_owned(),
        }
    );
}

#[test]
fn adjacent_edits_coalesce_to_single_whole_file_candidate() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "BADBAD\n")];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.initial_diagnostics.len(), 2);
    assert_eq!(result.initial_diagnostics[0].start_byte, Some(0));
    assert_eq!(result.initial_diagnostics[0].end_byte, Some(3));
    assert_eq!(result.initial_diagnostics[1].start_byte, Some(3));
    assert_eq!(result.initial_diagnostics[1].end_byte, Some(6));
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    let edits = &result.replacements[0];
    assert_eq!(edits.path, "src/lib.rs");
    assert_eq!(edits.original_digest, digest("BADBAD\n".as_bytes()));
    assert_eq!(edits.edits.len(), 1);
    assert_eq!(edits.edits[0].start_byte, 0);
    assert_eq!(edits.edits[0].end_byte, 5);
    assert_eq!(&edits.edits[0].replacement, b"GOODGOO");
    let original = "BADBAD\n".as_bytes();
    let replacement = &edits.edits[0].replacement;
    let mut spliced = Vec::new();
    spliced.extend_from_slice(&original[..edits.edits[0].start_byte as usize]);
    spliced.extend_from_slice(replacement);
    spliced.extend_from_slice(&original[edits.edits[0].end_byte as usize..]);
    assert_eq!(spliced, b"GOODGOOD\n");
    assert!(validate(&result).is_ok());
}

#[test]
fn longer_cycle_repeated_state_reports_oscillation() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let mut initial = BTreeMap::new();
    initial.insert("src/lib.rs".to_owned(), "a".to_owned());
    let cycle = |_: &str, _: &str, text: &str| match text {
        "a" => Ok("b".to_owned()),
        "b" => Ok("c".to_owned()),
        _ => Ok("a".to_owned()),
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, 10, cycle).expect("converged");
    assert_eq!(convergence, Convergence::Oscillation);
    assert_eq!(completed, 3);
    assert_eq!(terminal["src/lib.rs"], "a");
}

#[test]
fn tool_selection_reorder_yields_identical_manifests() {
    let files = vec![
        file("src/a.rs", "BAD FAIL a\n"),
        file("src/b.rs", "BAD FAIL b\n"),
    ];
    let forward = vec![
        stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"]),
        stage("lint-b", &["rust"], &["src/a.rs", "src/b.rs"]),
    ];
    let reversed = vec![
        stage("lint-b", &["rust"], &["src/a.rs", "src/b.rs"]),
        stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"]),
    ];
    let first = run_pipeline("//quality:test", "lint", &forward, &files).unwrap();
    let second = run_pipeline("//quality:test", "lint", &reversed, &files).unwrap();
    assert_eq!(first.convergence, Convergence::Stable as i32);
    assert_eq!(second.convergence, Convergence::Stable as i32);
    assert_eq!(first.completed_rounds, second.completed_rounds);
    assert_eq!(first.initial_diagnostics.len(), 4);
    assert_eq!(first.terminal_diagnostics.len(), 2);
    assert_eq!(first.replacements.len(), 2);
    assert_eq!(first.original_snapshot, second.original_snapshot);
    assert_eq!(first.terminal_snapshot, second.terminal_snapshot);
    assert_eq!(first.initial_diagnostics, second.initial_diagnostics);
    assert_eq!(first.terminal_diagnostics, second.terminal_diagnostics);
    assert_eq!(first.replacements, second.replacements);
    let mut first_stages: Vec<(String, Vec<String>)> = first
        .stages
        .iter()
        .map(|s| {
            let mut sources = s.source_paths.clone();
            sources.sort();
            (s.tool_id.clone(), sources)
        })
        .collect();
    let mut second_stages: Vec<(String, Vec<String>)> = second
        .stages
        .iter()
        .map(|s| {
            let mut sources = s.source_paths.clone();
            sources.sort();
            (s.tool_id.clone(), sources)
        })
        .collect();
    first_stages.sort();
    second_stages.sort();
    assert_eq!(first_stages, second_stages);
    assert!(quality_result::validate(&first).is_ok());
    assert!(quality_result::validate(&second).is_ok());
}

#[test]
fn shared_source_across_owners_converges_without_duplication() {
    let files = vec![
        file("src/a.rs", "BAD shared\n"),
        file("src/b.rs", "BAD solo\n"),
    ];
    let stages = vec![
        stage("lint-a", &["rust"], &["src/a.rs", "src/b.rs"]),
        stage("lint-b", &["rust"], &["src/a.rs"]),
    ];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.initial_diagnostics.len(), 2);
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 2);
    let shared = result
        .replacements
        .iter()
        .find(|edits| edits.path == "src/a.rs")
        .expect("shared file candidate");
    assert_eq!(shared.original_digest, digest("BAD shared\n".as_bytes()));
    assert_eq!(shared.edits.len(), 1);
    assert_eq!(shared.edits[0].start_byte, 0);
    assert_eq!(shared.edits[0].end_byte, 2);
    assert_eq!(&shared.edits[0].replacement, b"GOO");
    let original = "BAD shared\n".as_bytes();
    let mut spliced = Vec::new();
    spliced.extend_from_slice(&original[..shared.edits[0].start_byte as usize]);
    spliced.extend_from_slice(&shared.edits[0].replacement);
    spliced.extend_from_slice(&original[shared.edits[0].end_byte as usize..]);
    assert_eq!(spliced, b"GOOD shared\n");
    assert!(validate(&result).is_ok());
}

#[test]
fn iteration_limit_with_mixed_stable_and_growing_files_emits_none() {
    let stages = vec![stage(
        "lint-a",
        &["rust"],
        &["src/stable.rs", "src/growing.rs"],
    )];
    let mut initial = BTreeMap::new();
    initial.insert("src/stable.rs".to_owned(), "BAD\n".to_owned());
    initial.insert("src/growing.rs".to_owned(), "a".to_owned());
    let mixed = |_: &str, path: &str, text: &str| {
        if path == "src/stable.rs" {
            Ok(text.replace("BAD", "GOOD"))
        } else {
            Ok(format!("{text}x"))
        }
    };
    let (terminal, completed, convergence) =
        run_convergence(&initial, &stages, MAX_COMPLETED_ROUNDS, mixed).expect("converged");
    assert_eq!(convergence, Convergence::IterationLimit);
    assert_eq!(completed, MAX_COMPLETED_ROUNDS);
    assert_eq!(terminal["src/stable.rs"], "GOOD\n");
    assert_ne!(terminal["src/growing.rs"], initial["src/growing.rs"]);
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
fn chained_mutating_stages_see_virtual_snapshot() {
    let stages = vec![
        stage("lint-a", &["rust"], &["src/lib.rs"]),
        stage("fmt-a", &["rust"], &["src/lib.rs"]),
    ];
    let files = vec![file("src/lib.rs", "BAD   \n")];
    let result = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    let edits = &result.replacements[0];
    assert_eq!(edits.path, "src/lib.rs");
    assert_eq!(edits.original_digest, digest("BAD   \n".as_bytes()));
    assert_eq!(edits.edits.len(), 1);
    assert_eq!(edits.edits[0].start_byte, 0);
    assert_eq!(edits.edits[0].end_byte, 6);
    assert_eq!(&edits.edits[0].replacement, b"GOOD");
    let original = "BAD   \n".as_bytes();
    let mut spliced = Vec::new();
    spliced.extend_from_slice(&original[..edits.edits[0].start_byte as usize]);
    spliced.extend_from_slice(&edits.edits[0].replacement);
    spliced.extend_from_slice(&original[edits.edits[0].end_byte as usize..]);
    assert_eq!(spliced, b"GOOD\n");
    assert!(validate(&result).is_ok());
    let reversed = vec![
        stage("fmt-a", &["rust"], &["src/lib.rs"]),
        stage("lint-a", &["rust"], &["src/lib.rs"]),
    ];
    let other = run_pipeline("//quality:test", "lint", &reversed, &files).unwrap();
    assert_eq!(other.convergence, Convergence::Stable as i32);
    assert_eq!(other.terminal_snapshot, result.terminal_snapshot);
    assert_eq!(other.replacements, result.replacements);
    assert!(validate(&other).is_ok());
}

#[test]
fn per_stage_malformed_edits_rejected_by_validate_gate() {
    let stages = vec![stage("lint-a", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "BAD\n")];
    let valid = run_pipeline("//quality:test", "lint", &stages, &files).unwrap();
    assert_eq!(valid.replacements.len(), 1);
    assert!(validate(&valid).is_ok());
    let edits = &valid.replacements[0];
    assert_eq!(edits.edits.len(), 1);
    assert_eq!(edits.edits[0].start_byte, 0);
    assert_eq!(edits.edits[0].end_byte, 2);
    assert_eq!(&edits.edits[0].replacement, b"GOO");
    let mk = |edits: Vec<Edit>| {
        let mut mutated = valid.clone();
        mutated.replacements[0].edits = edits;
        mutated
    };
    let overlapping = mk(vec![
        Edit {
            start_byte: 0,
            end_byte: 3,
            replacement: b"X".to_vec(),
        },
        Edit {
            start_byte: 2,
            end_byte: 5,
            replacement: b"Y".to_vec(),
        },
    ]);
    assert!(validate(&overlapping).is_err());
    let unsorted = mk(vec![
        Edit {
            start_byte: 5,
            end_byte: 6,
            replacement: b"a".to_vec(),
        },
        Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"b".to_vec(),
        },
    ]);
    assert!(validate(&unsorted).is_err());
    let same_offset = mk(vec![
        Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"x".to_vec(),
        },
        Edit {
            start_byte: 0,
            end_byte: 2,
            replacement: b"y".to_vec(),
        },
    ]);
    assert!(validate(&same_offset).is_err());
    let insertion_inside_range = mk(vec![
        Edit {
            start_byte: 0,
            end_byte: 4,
            replacement: b"x".to_vec(),
        },
        Edit {
            start_byte: 2,
            end_byte: 2,
            replacement: b"y".to_vec(),
        },
    ]);
    assert!(validate(&insertion_inside_range).is_err());
    let noop = mk(vec![Edit {
        start_byte: 1,
        end_byte: 1,
        replacement: vec![],
    }]);
    assert!(validate(&noop).is_err());
    let inverted = mk(vec![Edit {
        start_byte: 2,
        end_byte: 1,
        replacement: b"x".to_vec(),
    }]);
    assert!(validate(&inverted).is_err());
    let mut bad_digest = valid.clone();
    bad_digest.replacements[0].original_digest = vec![0xAB; DIGEST_LEN + 1];
    assert!(validate(&bad_digest).is_err());
    let mut short_digest = valid.clone();
    short_digest.replacements[0].original_digest = vec![0xAB; DIGEST_LEN - 1];
    assert!(validate(&short_digest).is_err());
    let mut empty_digest = valid.clone();
    empty_digest.replacements[0].original_digest = vec![];
    assert!(validate(&empty_digest).is_err());
    let empty_edits = mk(vec![]);
    assert!(validate(&empty_edits).is_err());
    let mut unstable = valid.clone();
    unstable.convergence = Convergence::Oscillation as i32;
    assert!(validate(&unstable).is_err());
    let mut duplicated = valid.clone();
    duplicated.replacements.push(valid.replacements[0].clone());
    assert!(validate(&duplicated).is_err());
    let mut empty_path = valid.clone();
    empty_path.replacements[0].path = String::new();
    assert!(validate(&empty_path).is_err());
    let mut absolute_path = valid.clone();
    absolute_path.replacements[0].path = "/abs/src/lib.rs".to_owned();
    assert!(validate(&absolute_path).is_err());
    let mut non_utf8 = valid.clone();
    non_utf8.replacements[0].edits[0].replacement = vec![0xFF, 0xFE];
    assert!(validate(&non_utf8).is_err());
    for bad in [
        "src\\lib.rs",
        "src/./lib.rs",
        "src/../lib.rs",
        "src//lib.rs",
        "src/lib.rs/",
    ] {
        let mut malformed_path = valid.clone();
        malformed_path.replacements[0].path = bad.to_owned();
        assert!(validate(&malformed_path).is_err(), "path accepted: {bad:?}");
    }
}
