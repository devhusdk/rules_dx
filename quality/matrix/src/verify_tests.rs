//! Behavioral cases for the checks one matrix case must satisfy.

use quality_result::proto::QualityResult;
use quality_result::proto::{Capability, Convergence, Diagnostic, Edit, FileEdits, Stage};
use quality_result::{print_text, DIGEST_LEN, SCHEMA_MAJOR, SCHEMA_MINOR};

use crate::manifest::Manifest;
use crate::verify::{snapshot_diff, stage_update, verify_print, verify_result};
use crate::Error;

const MANIFEST: &str = r#"{
  "schema_version": 1,
  "name": "matrix_rust_lint_pass",
  "producer": "//quality/testdata:matrix_rust_lint_pass",
  "capability": "lint",
  "stages": ["clippy;rust;matrix/clippy_len.rs"],
  "runner": "_main/quality/runner/quality_runner",
  "printer": "_main/quality/result/print_result",
  "expected": "_main/expected.txt",
  "snapshot_dir": "quality/testdata/matrix"
}"#;

fn declared(capability: &str) -> Manifest {
    let text = MANIFEST.replace(
        "\"capability\": \"lint\"",
        &format!("\"capability\": \"{capability}\""),
    );
    Manifest::parse(text.as_bytes()).expect("manifest parses")
}

fn result() -> QualityResult {
    QualityResult {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        producer: "//quality/testdata:matrix_rust_lint_pass".to_owned(),
        capability: Capability::Lint as i32,
        stages: vec![Stage {
            tool_id: "clippy".to_owned(),
            class_ids: vec!["rust".to_owned()],
            source_paths: vec!["matrix/clippy_len.rs".to_owned()],
        }],
        completed_rounds: 1,
        convergence: Convergence::Stable as i32,
        original_snapshot: vec![],
        terminal_snapshot: vec![],
        initial_diagnostics: vec![Diagnostic {
            severity: 2,
            message: "length comparison to zero".to_owned(),
            tool_id: "clippy".to_owned(),
            rule_id: "clippy::len_zero".to_owned(),
            path: "matrix/clippy_len.rs".to_owned(),
            start_byte: Some(19),
            end_byte: Some(33),
            fixable: false,
        }],
        terminal_diagnostics: vec![],
        replacements: vec![FileEdits {
            path: "matrix/clippy_len.rs".to_owned(),
            original_digest: vec![0xAB; DIGEST_LEN],
            edits: vec![Edit {
                start_byte: 19,
                end_byte: 33,
                replacement: b"\"x\".is_empty()".to_vec(),
            }],
        }],
    }
}

/// One file rewritten by several edits, which the printed rows must cover.
fn multi_edit() -> QualityResult {
    let mut result = result();
    result.replacements.push(FileEdits {
        path: "matrix/other.rs".to_owned(),
        original_digest: vec![0x11; DIGEST_LEN],
        edits: vec![
            Edit {
                start_byte: 0,
                end_byte: 1,
                replacement: b"a".to_vec(),
            },
            Edit {
                start_byte: 4,
                end_byte: 5,
                replacement: b"b".to_vec(),
            },
        ],
    });
    result
}

#[test]
fn a_file_with_several_edits_is_counted_by_file_and_by_edit_separately() {
    let result = multi_edit();
    assert_eq!(result.replacements.len(), 2);
    assert_eq!(
        result
            .replacements
            .iter()
            .map(|file| file.edits.len())
            .sum::<usize>(),
        3
    );
    let canonical = print_text(&result);
    assert!(
        canonical.contains("replacements 2\n"),
        "the header counts files: {canonical}"
    );
    assert_eq!(
        canonical
            .lines()
            .filter(|line| line.starts_with("replacement ") && line.split_whitespace().count() > 2)
            .count(),
        3,
        "one row per edit: {canonical}"
    );
    verify_print(&canonical, &result).expect("canonical print");
    let lying = canonical.replace("replacements 2\n", "replacements 3\n");
    assert!(verify_print(&lying, &result).is_err());
    let lost_row = canonical.replacen("replacement matrix/other.rs 0 1", "", 1);
    assert!(verify_print(&lost_row, &result).is_err());
}

#[test]
fn a_result_that_matches_its_manifest_is_accepted() {
    let manifest = declared("lint");
    let result = result();
    verify_result(&manifest, &result).expect("matching result");
    verify_print(&print_text(&result), &result).expect("canonical print");
}

#[test]
fn a_result_from_another_producer_or_capability_is_rejected() {
    let manifest = declared("lint");
    let mut other = result();
    other.producer = "//quality/testdata:someone_else".to_owned();
    assert!(matches!(
        verify_result(&manifest, &other),
        Err(Error::ResultMismatch(_))
    ));
    let mut wrong = result();
    wrong.capability = Capability::Format as i32;
    assert!(matches!(
        verify_result(&manifest, &wrong),
        Err(Error::ResultMismatch(_))
    ));
    wrong.capability = 99;
    assert!(matches!(
        verify_result(&manifest, &wrong),
        Err(Error::ResultMismatch(_))
    ));
    assert!(matches!(
        verify_result(&declared("audit"), &result()),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn a_stage_count_or_shape_mismatch_is_rejected() {
    let manifest = declared("lint");
    let mut fewer = result();
    fewer.stages.clear();
    assert!(matches!(
        verify_result(&manifest, &fewer),
        Err(Error::ResultMismatch(_))
    ));
    let mut renamed = result();
    renamed.stages[0].tool_id = "rustc".to_owned();
    assert!(matches!(
        verify_result(&manifest, &renamed),
        Err(Error::ResultMismatch(_))
    ));
    let mut extra_class = result();
    extra_class.stages[0].class_ids.push("toml".to_owned());
    assert!(matches!(
        verify_result(&manifest, &extra_class),
        Err(Error::ResultMismatch(_))
    ));
    let mut moved = result();
    moved.stages[0].source_paths = vec!["other.rs".to_owned()];
    assert!(matches!(
        verify_result(&manifest, &moved),
        Err(Error::ResultMismatch(_))
    ));
}

#[test]
fn a_result_that_did_not_converge_or_never_ran_is_rejected() {
    let manifest = declared("lint");
    let mut oscillating = result();
    oscillating.convergence = Convergence::Oscillation as i32;
    assert!(matches!(
        verify_result(&manifest, &oscillating),
        Err(Error::ResultMismatch(_))
    ));
    let mut never = result();
    never.completed_rounds = 0;
    assert!(matches!(
        verify_result(&manifest, &never),
        Err(Error::ResultMismatch(_))
    ));
}

#[test]
fn a_print_that_disagrees_with_its_result_is_rejected() {
    let result = result();
    let canonical = print_text(&result);
    assert!(verify_print(&canonical, &result).is_ok());

    let without_newline = canonical.trim_end().to_owned();
    assert!(verify_print(&without_newline, &result).is_err());
    assert!(verify_print(&format!("{canonical}extra\n"), &result).is_err());

    let mut dropped_row = canonical.clone();
    dropped_row = dropped_row.replace("initial WARNING", "initial SKIPPED");
    assert!(verify_print(&dropped_row, &result).is_err());

    let mut dropped_header = canonical.clone();
    dropped_header = dropped_header.replace("terminal 0\n", "");
    assert!(verify_print(&dropped_header, &result).is_err());

    let mut wrong_count = canonical.clone();
    wrong_count = wrong_count.replace("initial 1\n", "initial 2\n");
    assert!(verify_print(&wrong_count, &result).is_err());

    let mut wrong_rounds = canonical.clone();
    wrong_rounds = wrong_rounds.replace("completed_rounds 1\n", "completed_rounds 3\n");
    assert!(verify_print(&wrong_rounds, &result).is_err());

    let mut wrong_stages = canonical.clone();
    wrong_stages = wrong_stages.replace("stages 1\n", "stages 4\n");
    assert!(verify_print(&wrong_stages, &result).is_err());

    let mut dropped_stage_row = canonical.clone();
    dropped_stage_row = dropped_stage_row.replacen(
        "stage clippy classes=rust sources=matrix/clippy_len.rs\n",
        "",
        1,
    );
    assert!(verify_print(&dropped_stage_row, &result).is_err());
}

#[test]
fn a_snapshot_difference_names_both_sides() {
    let rendered = snapshot_diff("producer //a\n", "producer //b\n").expect("diff renders");
    assert!(
        rendered.contains("producer //a"),
        "expected side: {rendered}"
    );
    assert!(rendered.contains("producer //b"), "actual side: {rendered}");
    assert!(
        rendered.contains('-') && rendered.contains('+'),
        "both sides are marked: {rendered}"
    );
    assert!(
        snapshot_diff("same\n", "same\n").is_err(),
        "an identical pair is not a difference to render"
    );
}

#[test]
fn a_staged_update_lands_under_the_reviewed_name_and_leaves_the_snapshot_alone() {
    let dir = tempfile::Builder::new()
        .prefix("dx-matrix-update-")
        .tempdir()
        .expect("scratch");
    let target = dir.path().join("nested");
    let staged = stage_update(&target, "case.expected.update", b"fresh\n").expect("staged");
    assert_eq!(staged, target.join("case.expected.update"));
    assert_eq!(std::fs::read(&staged).expect("staged bytes"), b"fresh\n");
    assert!(stage_update(dir.path(), "case.expected.update", b"").is_ok());
    let file_instead_of_dir = dir.path().join("plain");
    std::fs::write(&file_instead_of_dir, b"x").expect("blocking file");
    assert!(
        stage_update(&file_instead_of_dir, "case.expected.update", b"").is_err(),
        "a parent that is not a directory cannot receive the update"
    );
}
