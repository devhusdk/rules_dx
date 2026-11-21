use super::{
    Collected, Unstable, collect_results_in, map_diagnostic, map_severity, stage_artifact,
    stage_result,
};
use dx_exec_core::common::{FileChange, apply_to_bytes};
use dx_exec_quality_testing::{LAST_EVENT, QualityWorkspace, diagnostic, result_full, temp_dir};
use dx_output::{DiagnosticEvent, Severity, Snapshot};
use quality_result::decode_validated;
use quality_result::{encode_validated, proto};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[test]
fn keep_paths_drops_other_files() {
    let mut collected = Collected {
        tools: Vec::new(),
        initial: Vec::new(),
        terminal: Vec::new(),
        changes: Vec::new(),
        terminal_digests: BTreeMap::new(),
        original_digests: BTreeMap::new(),
        conflicts: Vec::new(),
        unstable: Vec::new(),
        complete: true,
        problems: Vec::new(),
    };
    for path in ["src/a.py", "src/b.py"] {
        let mut event = map_diagnostic(&diagnostic("m", false), Snapshot::Initial, Path::new("."))
            .expect("event");
        event.path = Some(path.to_owned());
        collected.initial.push(event);
        collected.changes.push(FileChange {
            path: path.to_owned(),
            original_digest: [0; 32],
            edits: Vec::new(),
        });
        collected.original_digests.insert(path.to_owned(), [0; 32]);
    }
    collected.keep_paths(&["src/a.py".to_owned()]);
    assert_eq!(collected.initial.len(), 1);
    assert_eq!(collected.initial[0].path.as_deref(), Some("src/a.py"));
    assert_eq!(collected.changes.len(), 1);
    assert_eq!(collected.changes[0].path, "src/a.py");
    assert_eq!(collected.original_digests.len(), 1);
}

#[test]
fn severity_mapping_covers_all_arms() {
    assert!(map_severity(proto::Severity::Unspecified as i32).is_none());
    assert!(matches!(
        map_severity(proto::Severity::Info as i32),
        Some(Severity::Info)
    ));
    assert!(matches!(
        map_severity(proto::Severity::Warning as i32),
        Some(Severity::Warning)
    ));
    assert!(matches!(
        map_severity(proto::Severity::Error as i32),
        Some(Severity::Error)
    ));
    assert!(map_severity(99).is_none());
}

#[test]
fn diagnostic_mapping_rejects_bad_shapes() {
    let base = diagnostic("m", false);
    let mut partial = base.clone();
    partial.end_byte = None;
    assert!(map_diagnostic(&partial, Snapshot::Initial, Path::new(".")).is_none());
    let mut pathless = base.clone();
    pathless.path = String::new();
    assert!(map_diagnostic(&pathless, Snapshot::Initial, Path::new(".")).is_none());
    assert!(map_diagnostic(&pathless, Snapshot::Terminal, Path::new(".")).is_none());
    let mut stripped = pathless.clone();
    stripped.start_byte = None;
    stripped.end_byte = None;
    assert!(map_diagnostic(&stripped, Snapshot::Terminal, Path::new(".")).is_some());
}

#[test]
fn unused_dependency_is_placed_on_its_build_line() {
    let root = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(root.path().join("p")).expect("package");
    let build = "rust_library(\n    name = \"lib\",\n    deps = [\"//a:a\"],\n)\n";
    std::fs::write(root.path().join("p/BUILD.bazel"), build).expect("build");
    let mut diagnostic = diagnostic("unused", false);
    diagnostic.path = String::new();
    diagnostic.start_byte = None;
    diagnostic.end_byte = None;
    diagnostic.build_dep = Some(proto::BuildDep {
        package: "p".to_owned(),
        target: "lib".to_owned(),
        dependency: "a".to_owned(),
        ..Default::default()
    });
    let placed = map_diagnostic(&diagnostic, Snapshot::Terminal, root.path()).expect("mapped");
    let (start, end) = placed.range.expect("range");
    assert_eq!(placed.path.as_deref(), Some("p/BUILD.bazel"));
    assert_eq!(
        build.get(usize::try_from(start).expect("start")..usize::try_from(end).expect("end")),
        Some("\"//a:a\"")
    );

    std::fs::remove_file(root.path().join("p/BUILD.bazel")).expect("remove");
    let kept = map_diagnostic(&diagnostic, Snapshot::Terminal, root.path()).expect("mapped");
    assert!(kept.path.is_none() && kept.range.is_none());
}

#[test]
fn staging_rejects_results_the_report_cannot_carry() {
    let harness = QualityWorkspace::new("stage-reject");
    harness.write_source("src/a.py", "x = 1\n");
    let bytes = harness.valid_result(
        vec![diagnostic("unused", false)],
        vec![harness.replacement(b"y")],
    );
    let mut result = decode_validated(&bytes).expect("valid result");
    result.terminal_diagnostics = result.initial_diagnostics.clone();
    assert!(
        stage_artifact(&bytes, Path::new(".")).is_some(),
        "a valid artifact stages"
    );
    assert!(
        stage_result(&result, Path::new(".")).is_some(),
        "a valid result stages"
    );
    assert!(stage_artifact(b"not-a-validated-result", Path::new(".")).is_none());

    let mut short_snapshot = result.clone();
    short_snapshot.terminal_snapshot[0].digest.pop();
    let mut no_severity = result.clone();
    no_severity.initial_diagnostics[0].severity = proto::Severity::Unspecified as i32;
    let mut no_end = result.clone();
    no_end.terminal_diagnostics[0].end_byte = None;
    let mut short_original = result.clone();
    short_original.replacements[0].original_digest.pop();
    for broken in [short_snapshot, no_severity, no_end, short_original] {
        assert!(stage_result(&broken, Path::new(".")).is_none());
        assert!(
            encode_validated(&broken).is_err(),
            "a producer cannot send what staging rejects"
        );
    }
}

#[test]
fn staging_rejects_non_utf8_replacements() {
    let harness = QualityWorkspace::new("stage-bytes");
    harness.write_source("src/a.py", "x = 1\n");
    let bytes = harness.valid_result(vec![], vec![harness.replacement(b"y")]);
    let mut result = decode_validated(&bytes).expect("valid result");
    result.replacements[0].edits[0].replacement = vec![0xFF];
    assert!(
        stage_result(&result, Path::new(".")).is_none(),
        "a change whose replacement is not text never reaches the report"
    );
    assert!(
        encode_validated(&result).is_err(),
        "a producer cannot send what staging rejects"
    );
}

#[test]
fn unstable_description_names_target_configuration_and_cause() {
    let mut item = Unstable {
        label: "//a:one".to_owned(),
        configuration: Some("cfg1".to_owned()),
        producer: "@@//a:one".to_owned(),
        convergence: "ITERATION_LIMIT",
        rounds: 10,
        tools: vec!["biome".to_owned(), "prettier".to_owned()],
    };
    assert_eq!(
        item.describe(),
        "//a:one in configuration cfg1 did not converge: \
         ITERATION_LIMIT after 10 round(s) of biome, prettier"
    );
    item.producer = "//b:other".to_owned();
    item.configuration = None;
    assert_eq!(
        item.describe(),
        "//a:one (//b:other) did not converge: \
         ITERATION_LIMIT after 10 round(s) of biome, prettier"
    );
}

#[test]
fn invalid_output_group_rejected() {
    let dir = temp_dir("group-tmp");
    let bep = dir.path().join("empty.json");
    std::fs::write(&bep, "").expect("bep");
    let Err(error) = collect_results_in(&bep, &BTreeSet::new(), "", dir.path(), &|| None) else {
        panic!("empty output group must fail"); // LCOV_EXCL_LINE - reason: defensive branch, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    };
    assert!(error.message().contains("invalid BEP config"));
}

const ASPECT: &str = "//core/quality:families/core.bzl%real_lint_aspect";

fn completion_id(label: &str) -> serde_json::Value {
    serde_json::json!({"targetCompleted": {
        "label": label,
        "aspect": ASPECT,
        "configuration": {"id": "cfg1"},
    }})
}

fn configured(labels: &[&str]) -> String {
    let children: Vec<serde_json::Value> =
        labels.iter().map(|label| completion_id(label)).collect();
    serde_json::json!({
        "id": {"targetConfigured": {"label": labels[0], "aspect": ASPECT}},
        "children": children,
    })
    .to_string()
}

fn produced(harness: &QualityWorkspace, label: &str, set: &str) -> Vec<String> {
    let artifact = harness.temp.join(format!("{set}.pb"));
    std::fs::write(&artifact, harness.valid_result(vec![], vec![])).expect("artifact");
    vec![
        serde_json::json!({
            "id": {"namedSet": {"id": set}},
            "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
        })
        .to_string(),
        serde_json::json!({
            "id": completion_id(label),
            "completed": {
                "success": true,
                "outputGroup": [{"name": "dx_results", "fileSets": [{"id": set}]}],
            },
        })
        .to_string(),
    ]
}

fn collect(harness: &QualityWorkspace, lines: &[String]) -> Collected {
    let bep = harness.temp.join("events.json");
    std::fs::write(&bep, lines.join("\n")).expect("bep");
    collect_results_in(
        &bep,
        &BTreeSet::new(),
        "dx_results",
        &harness.workspace,
        &|| None,
    )
    .expect("collected")
}

#[test]
fn two_producers_both_reported_are_complete() {
    let harness = QualityWorkspace::new("producers-both");
    harness.write_source("src/a.py", "x = 1\n");
    let mut lines = vec![configured(&["//a:one", "//a:two"])];
    lines.extend(produced(&harness, "//a:one", "0"));
    lines.extend(produced(&harness, "//a:two", "1"));
    lines.push(LAST_EVENT.to_owned());
    let collected = collect(&harness, &lines);
    assert!(collected.complete, "{:?}", collected.problems);
    assert!(collected.problem_detail().is_none());
}

#[test]
fn an_announced_producer_that_never_reports_is_incomplete() {
    let harness = QualityWorkspace::new("producers-one-missing");
    harness.write_source("src/a.py", "x = 1\n");
    let mut lines = vec![configured(&["//a:one", "//a:two"])];
    lines.extend(produced(&harness, "//a:one", "0"));
    lines.push(LAST_EVENT.to_owned());
    let collected = collect(&harness, &lines);
    assert!(!collected.complete);
    assert_eq!(
        collected.problem_detail().as_deref(),
        Some(
            "targetCompleted for //a:two (//core/quality:families/core.bzl%real_lint_aspect) \
             was announced but never reported"
        )
    );
}

#[test]
fn a_stream_without_bazels_last_event_is_incomplete() {
    let harness = QualityWorkspace::new("producers-unterminated");
    harness.write_source("src/a.py", "x = 1\n");
    let lines = produced(&harness, "//a:one", "0");
    let collected = collect(&harness, &lines);
    assert!(!collected.complete);
    assert_eq!(
        collected.problems,
        vec!["build events ended before Bazel's last event".to_owned()]
    );
    let empty = collect(&harness, &[]);
    assert!(!empty.complete, "an empty stream proves nothing");
}

#[test]
fn no_applicable_work_is_complete_only_once_every_target_reported() {
    let harness = QualityWorkspace::new("producers-no-work");
    let done = serde_json::json!({
        "id": {"targetCompleted": {"label": "//a:docs", "configuration": {"id": "cfg1"}}},
        "completed": {"success": true},
    })
    .to_string();
    let announce = serde_json::json!({
        "id": {"targetConfigured": {"label": "//a:docs"}},
        "children": [{"targetCompleted": {"label": "//a:docs", "configuration": {"id": "cfg1"}}}],
    })
    .to_string();
    let clean = collect(
        &harness,
        &[announce.clone(), done.clone(), LAST_EVENT.to_owned()],
    );
    assert!(clean.complete, "{:?}", clean.problems);
    assert!(clean.tools.is_empty(), "{:?}", clean.tools);
    let missing = collect(&harness, &[announce, LAST_EVENT.to_owned()]);
    assert!(!missing.complete);
}

#[test]
fn skipped_targets_are_not_missing_results() {
    let harness = QualityWorkspace::new("producers-skipped");
    harness.write_source("src/a.py", "x = 1\n");
    let mut lines = vec![configured(&["//a:one", "//a:windows_only"])];
    lines.extend(produced(&harness, "//a:one", "0"));
    lines.push(
        serde_json::json!({"id": completion_id("//a:windows_only"), "aborted": {"reason": "SKIPPED"}})
            .to_string(),
    );
    lines.push(LAST_EVENT.to_owned());
    let collected = collect(&harness, &lines);
    assert!(collected.complete, "{:?}", collected.problems);
}

#[test]
fn nobuild_targets_are_not_missing_results() {
    let harness = QualityWorkspace::new("producers-nobuild");
    let mut lines = vec![configured(&["//a:one"])];
    lines.push(
        serde_json::json!({"id": completion_id("//a:one"), "aborted": {"reason": "NO_BUILD"}})
            .to_string(),
    );
    lines.push(LAST_EVENT.to_owned());
    let collected = collect(&harness, &lines);
    assert!(collected.complete, "{:?}", collected.problems);
}

#[test]
fn aborted_and_incomplete_producers_name_their_cause() {
    let harness = QualityWorkspace::new("producers-aborted");
    harness.write_source("src/a.py", "x = 1\n");
    let mut lines = vec![configured(&["//a:one", "//a:two"])];
    lines.extend(produced(&harness, "//a:one", "0"));
    lines.push(
        serde_json::json!({"id": completion_id("//a:two"), "aborted": {"reason": "USER_INTERRUPTED"}})
            .to_string(),
    );
    lines.push(LAST_EVENT.to_owned());
    let collected = collect(&harness, &lines);
    assert!(!collected.complete);
    assert!(
        collected.problems[0].ends_with("was aborted: USER_INTERRUPTED"),
        "{:?}",
        collected.problems
    );

    let partial = serde_json::json!({
        "id": completion_id("//a:one"),
        "completed": {
            "success": true,
            "outputGroup": [{"name": "dx_results", "incomplete": true}],
        },
    })
    .to_string();
    let collected = collect(&harness, &[partial, LAST_EVENT.to_owned()]);
    assert!(!collected.complete);
    assert_eq!(
        collected.problems,
        vec!["//a:one reported an incomplete dx_results output group".to_owned()]
    );
}

#[test]
fn an_invalid_result_names_its_producer() {
    let harness = QualityWorkspace::new("producers-invalid");
    let artifact = harness.temp.join("bad.pb");
    std::fs::write(&artifact, b"not a result").expect("artifact");
    let lines = vec![
        serde_json::json!({
            "id": {"namedSet": {"id": "0"}},
            "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
        })
        .to_string(),
        serde_json::json!({
            "id": completion_id("//a:one"),
            "completed": {
                "success": true,
                "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
            },
        })
        .to_string(),
        LAST_EVENT.to_owned(),
    ];
    let collected = collect(&harness, &lines);
    assert!(!collected.complete);
    assert_eq!(
        collected.problems,
        vec!["//a:one reported an invalid quality result".to_owned()]
    );
}

#[test]
fn a_cut_off_last_line_is_incomplete_not_invalid() {
    let harness = QualityWorkspace::new("producers-cut-off");
    harness.write_source("src/a.py", "x = 1\n");
    let mut lines = produced(&harness, "//a:one", "0");
    lines.push(r#"{"id":{"targetCo"#.to_owned());
    let collected = collect(&harness, &lines);
    assert!(!collected.complete);
    assert!(
        collected.problems[0].starts_with("build events cut off at line 3: invalid JSON"),
        "{:?}",
        collected.problems
    );
    assert!(
        collected
            .problems
            .contains(&"build events ended before Bazel's last event".to_owned()),
        "{:?}",
        collected.problems
    );
    assert!(!collected.tools.is_empty(), "{:?}", collected.tools);
}

#[test]
fn an_artifact_reported_by_two_targets_is_read_once() {
    let harness = QualityWorkspace::new("producers-shared-artifact");
    harness.write_source("src/a.py", "x = 1\n");
    let artifact = harness.temp.join("shared.pb");
    let bytes = harness.valid_result(vec![diagnostic("unused", false)], vec![]);
    std::fs::write(&artifact, bytes).expect("artifact");
    let report = |label: &str| {
        serde_json::json!({
            "id": completion_id(label),
            "completed": {
                "success": true,
                "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
            },
        })
        .to_string()
    };
    let lines = vec![
        serde_json::json!({
            "id": {"namedSet": {"id": "0"}},
            "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
        })
        .to_string(),
        report("//a:x"),
        report("//a:x_test"),
        LAST_EVENT.to_owned(),
    ];
    let once = collect(&harness, &lines[..3]);
    let twice = collect(&harness, &lines);
    assert_eq!(once.initial.len(), twice.initial.len());
    assert!(!twice.initial.is_empty(), "expected non-empty");
}

#[test]
fn a_bad_line_mid_stream_is_reported_not_fatal() {
    let harness = QualityWorkspace::new("producers-mid-stream");
    let mut lines = vec!["{\"id\":{\"targetCo".to_owned()];
    lines.extend(produced(&harness, "//a:one", "0"));
    lines.push(LAST_EVENT.to_owned());
    let bep = harness.temp.join("events.json");
    std::fs::write(&bep, lines.join("\n")).expect("bep");
    let collected = collect_results_in(
        &bep,
        &BTreeSet::new(),
        "dx_results",
        &harness.workspace,
        &|| None,
    )
    .expect("a garbled line must not fail the run");
    assert!(
        collected
            .problems
            .iter()
            .any(|p| p.contains("line 1: invalid JSON")),
        "{:?}",
        collected.problems
    );
    assert!(!collected.tools.is_empty(), "{:?}", collected.tools);
}

const TWO_DEPS: &str = "rust_library(\n    name = \"one\",\n    deps = [\"//a:a\"],\n)\n\nrust_library(\n    name = \"two\",\n    deps = [\"//b:b\"],\n)\n";

fn unused(target: &str, dependency: &str) -> proto::Diagnostic {
    let mut diagnostic = diagnostic("unused", false);
    diagnostic.path = String::new();
    diagnostic.start_byte = None;
    diagnostic.end_byte = None;
    diagnostic.build_dep = Some(proto::BuildDep {
        package: "p".to_owned(),
        target: target.to_owned(),
        dependency: dependency.to_owned(),
        ..Default::default()
    });
    diagnostic
}

fn collect_artifacts(harness: &QualityWorkspace, artifacts: &[(&str, Vec<u8>)]) -> Collected {
    let mut lines = vec![configured(
        &artifacts
            .iter()
            .map(|(label, _)| *label)
            .collect::<Vec<_>>(),
    )];
    for (index, (label, bytes)) in artifacts.iter().enumerate() {
        let set = index.to_string();
        let artifact = harness.temp.join(format!("{set}.pb"));
        std::fs::write(&artifact, bytes).expect("artifact");
        lines.push(
            serde_json::json!({
                "id": {"namedSet": {"id": set}},
                "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
            })
            .to_string(),
        );
        lines.push(
            serde_json::json!({
                "id": completion_id(label),
                "completed": {
                    "success": true,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": set}]}],
                },
            })
            .to_string(),
        );
    }
    lines.push(LAST_EVENT.to_owned());
    collect(harness, &lines)
}

fn unused_deps_artifact(deps: &[(&str, &str)]) -> Vec<u8> {
    let found: Vec<proto::Diagnostic> = deps.iter().map(|(t, d)| unused(t, d)).collect();
    result_full(found.clone(), found, vec![], vec![])
}

#[test]
fn unused_deps_in_one_build_file_become_one_change() {
    let harness = QualityWorkspace::new("unused-deps-one");
    harness.write_source("p/BUILD.bazel", TWO_DEPS);
    let bytes = unused_deps_artifact(&[("one", "a"), ("two", "b"), ("one", "a")]);
    let collected = collect_artifacts(&harness, &[("//p:one", bytes)]);
    assert!(collected.conflicts.is_empty());
    assert_eq!(collected.changes.len(), 1);
    let change = &collected.changes[0];
    assert_eq!(change.path, "p/BUILD.bazel");
    let after = apply_to_bytes(TWO_DEPS.as_bytes(), &change.edits).expect("applied");
    let after = String::from_utf8(after).expect("utf8");
    assert!(
        !after.contains("//a:a") && !after.contains("//b:b"),
        "{after}"
    );
    assert!(
        collected
            .terminal
            .iter()
            .all(|e| e.path.as_deref() != Some("p/BUILD.bazel"))
    );
    assert!(collected.initial.iter().all(|e| e.fixable));
}

fn missing(target: &str, label: &str) -> proto::Diagnostic {
    let mut diagnostic = unused(target, "");
    if let Some(dep) = diagnostic.build_dep.as_mut() {
        dep.missing = true;
        dep.label = label.to_owned();
    }
    diagnostic.message = format!("missing {label}");
    diagnostic
}

#[test]
fn missing_dep_is_added_while_the_only_entry_is_removed() {
    let harness = QualityWorkspace::new("missing-dep-swap");
    harness.write_source("p/BUILD.bazel", TWO_DEPS);
    let found = vec![
        unused("one", "a"),
        missing("one", "@@//q:needed"),
        missing("two", "@@x//:y"),
    ];
    let bytes = result_full(found.clone(), found, vec![], vec![]);
    let collected = collect_artifacts(&harness, &[("//p:one", bytes)]);
    assert_eq!(collected.changes.len(), 1);
    let change = &collected.changes[0];
    let after = apply_to_bytes(TWO_DEPS.as_bytes(), &change.edits).expect("applied");
    let after = String::from_utf8(after).expect("utf8");
    assert!(after.contains("deps = [\"//q:needed\"]"), "{after}");
    assert!(
        !after.contains("//a:a") && after.contains("//b:b"),
        "{after}"
    );
    let on_list = |event: &DiagnosticEvent| event.message.contains("@@x//:y");
    let external = collected
        .initial
        .iter()
        .find(|e| on_list(e))
        .expect("event");
    assert!(external.path.is_some() && !external.fixable);
    assert!(
        collected
            .initial
            .iter()
            .filter(|e| !on_list(e))
            .all(|e| e.fixable)
    );
}

#[test]
fn unused_dep_edit_conflicts_with_another_editor() {
    let harness = QualityWorkspace::new("unused-deps-conflict");
    harness.write_source("p/BUILD.bazel", TWO_DEPS);
    let digest = dx_digest::blake3(TWO_DEPS.as_bytes()).to_vec();
    let snapshot = proto::FileSnapshot {
        path: "p/BUILD.bazel".to_owned(),
        digest,
    };
    let editor = result_full(
        vec![],
        vec![],
        vec![harness.replacement_in("p/BUILD.bazel", b"X")],
        vec![snapshot],
    );
    let unused = unused_deps_artifact(&[("one", "a")]);
    let collected = collect_artifacts(&harness, &[("//p:fmt", editor), ("//p:one", unused)]);
    assert!(collected.changes.is_empty());
    assert_eq!(collected.conflicts.len(), 1);
    assert_eq!(collected.conflicts[0].path, "p/BUILD.bazel");
}

#[test]
fn rewritten_build_file_makes_the_unused_dep_change_stale() {
    let harness = QualityWorkspace::new("unused-deps-stale");
    harness.write_source("p/BUILD.bazel", TWO_DEPS);
    let bytes = unused_deps_artifact(&[("one", "a")]);
    let collected = collect_artifacts(&harness, &[("//p:one", bytes)]);
    harness.write_source("p/BUILD.bazel", "# changed\n");
    let outcome = crate::quality_apply::apply_collected_changes(
        &dx_apply::RealFileSystem,
        &harness.workspace,
        true,
        true,
        None,
        &collected.changes,
    );
    assert_eq!(outcome.applied.get("p/BUILD.bazel"), Some(&false));
    assert_eq!(
        outcome.not_applied,
        vec![("p/BUILD.bazel".to_owned(), "stale_source")]
    );
}
