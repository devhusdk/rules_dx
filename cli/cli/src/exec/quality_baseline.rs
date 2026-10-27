use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_apply::{FileSystem, RealFileSystem};
use dx_fingerprint::baseline::{
    describe_entry, match_baseline, parse_baseline, refresh_baseline, render_baseline,
    unidentified, AnalyzedScope, BaselineFile, Finding,
};
use dx_output::{DiagnosticEvent, Snapshot};

use super::results::Collected;
use crate::reports::finding_identity;

use super::common::CODE_VERIFICATION_FAILED;

pub(crate) struct BaselineView {
    pub(crate) rel: String,
    pub(crate) suppressed: Vec<bool>,
    pub(crate) total: u64,
    pub(crate) fresh: u64,
    pub(crate) held: u64,
    pub(crate) stale: Vec<String>,
    pub(crate) refreshed: bool,
}

impl BaselineView {
    pub(crate) fn counts(&self) -> [u64; 3] {
        [self.total, self.fresh, self.held]
    }
}

fn finding_for(
    diagnostic: &DiagnosticEvent,
    files: &BTreeMap<String, Option<Vec<u8>>>,
    originals: &BTreeMap<String, [u8; 32]>,
) -> Finding {
    let drifted = diagnostic.snapshot == Snapshot::Initial
        && diagnostic.path.as_ref().is_some_and(|path| {
            match (
                files.get(path).and_then(|bytes| bytes.as_ref()),
                originals.get(path),
            ) {
                (Some(bytes), Some(want)) => dx_digest::blake3(bytes) != *want,
                _ => false,
            }
        });
    let bytes = diagnostic
        .path
        .as_ref()
        .filter(|_| !drifted)
        .and_then(|path| files.get(path))
        .and_then(|bytes| bytes.as_deref());
    finding_identity(diagnostic, bytes)
}

fn read_sources(workspace: &Path, status: &[DiagnosticEvent]) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut paths = BTreeSet::new();
    for diagnostic in status {
        if let Some(path) = &diagnostic.path {
            paths.insert(path.clone());
        }
    }
    let mut files = BTreeMap::new();
    for path in paths {
        files.insert(path.clone(), std::fs::read(workspace.join(&path)).ok());
    }
    files
}

pub(crate) fn analyzed_scope(collected: &Collected) -> AnalyzedScope {
    AnalyzedScope {
        ran: collected.ran.clone(),
        pairs: collected.analyzed.clone(),
    }
}

fn write_baseline(full: &Path, rendered: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = full.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    RealFileSystem.write_atomic(full, rendered)
}

fn view_for(
    rel: &str,
    baseline: &BaselineFile,
    findings: &[Finding],
    scope: &AnalyzedScope,
    complete: bool,
    refreshed: bool,
) -> BaselineView {
    let matched = match_baseline(baseline, findings, scope, complete);
    let held = matched.suppressed.iter().filter(|done| **done).count() as u64;
    BaselineView {
        rel: rel.to_owned(),
        suppressed: matched.suppressed,
        total: findings.len() as u64,
        fresh: findings.len() as u64 - held,
        held,
        stale: matched.stale.iter().map(describe_entry).collect(),
        refreshed,
    }
}

/// What a run must show before `--apply` may rewrite the baseline file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BaselineGate {
    /// Every target reported results.
    pub(crate) complete: bool,
    /// Every result converged.
    pub(crate) stable: bool,
    /// Every requested change was written.
    pub(crate) applied_all: bool,
}

impl BaselineGate {
    /// The first reason the baseline must not be refreshed, if any.
    pub(crate) fn refresh_blocked(self) -> Option<&'static str> {
        if !self.complete {
            Some("analysis was incomplete")
        } else if !self.stable {
            Some("a result did not converge")
        } else if !self.applied_all {
            Some("a fix was not applied")
        } else {
            None
        }
    }

    /// Whether the findings describe the whole analyzed scope.
    fn matching_complete(self) -> bool {
        self.complete && self.stable
    }
}

pub(crate) fn apply_baseline(
    workspace: &Path,
    status: &[DiagnosticEvent],
    scope: &AnalyzedScope,
    originals: &BTreeMap<String, [u8; 32]>,
    gate: BaselineGate,
    apply: bool,
    baseline: Option<&str>,
) -> Result<Option<BaselineView>, (String, String)> {
    let fail = |message: String| (CODE_VERIFICATION_FAILED.to_owned(), message);
    let Some(rel) = baseline else {
        return Ok(None);
    };
    let full = dx_adopt::join_quality_baseline(workspace, rel);
    let bytes = match std::fs::read(&full) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(fail(format!("cannot read baseline {rel}: {error}"))),
    };
    let files = read_sources(workspace, status);
    let findings: Vec<Finding> = status
        .iter()
        .map(|diagnostic| finding_for(diagnostic, &files, originals))
        .collect();
    let blocked = gate.refresh_blocked();
    let write = apply && blocked.is_none();
    let complete = gate.matching_complete();
    if write {
        let blind = unidentified(&findings);
        if let Some(first) = blind.first() {
            return Err(fail(format!(
                "baseline {rel} was not refreshed: source identity is unavailable for {first}"
            )));
        }
    }
    let Some(bytes) = bytes else {
        if let (true, Some(cause)) = (apply, blocked) {
            return Err(fail(format!(
                "baseline {rel} is missing and was not recorded: {cause}"
            )));
        }
        if !apply {
            return Err(fail(format!(
                "baseline {rel} is missing: run with --apply to record current findings"
            )));
        }
        let next = refresh_baseline(&BaselineFile::default(), &findings, scope);
        let rendered = render_baseline(&next).map_err(|error| fail(error.to_string()))?;
        write_baseline(&full, rendered.as_bytes())
            .map_err(|error| fail(format!("cannot write baseline {rel}: {error}")))?;
        return Ok(Some(view_for(rel, &next, &findings, scope, complete, true)));
    };
    let baseline = parse_baseline(&bytes).map_err(|error| fail(format!("{rel}: {error}")))?;
    if !write {
        return Ok(Some(view_for(
            rel, &baseline, &findings, scope, complete, false,
        )));
    }
    let next = refresh_baseline(&baseline, &findings, scope);
    let rendered = render_baseline(&next).map_err(|error| fail(error.to_string()))?;
    write_baseline(&full, rendered.as_bytes())
        .map_err(|error| fail(format!("cannot write baseline {rel}: {error}")))?;
    Ok(Some(view_for(rel, &next, &findings, scope, complete, true)))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{json_events, FailingWrites, Harness};
    use super::*;
    use dx_fingerprint::baseline::{
        normalize_message, render_baseline, BaselineEntry, BaselineFile,
    };
    use quality_result::proto::{Capability, Convergence, FileSnapshot, QualityResult, Stage};
    use quality_result::{encode_validated, SCHEMA_MAJOR, SCHEMA_MINOR};
    use std::collections::BTreeMap;

    const TOOL: &str = "lint-tool";
    const RULE: &str = "lint-tool/rule";
    const PATH: &str = "src/a.py";
    const BODY: &str = "x = 1\n";
    const LINE: &[u8] = b"x = 1";
    const REL: &str = "quality/baseline.json";

    fn recorded(message: &str, context: &[u8]) -> BaselineEntry {
        BaselineEntry {
            tool: TOOL.to_owned(),
            rule: RULE.to_owned(),
            path: PATH.to_owned(),
            target: String::new(),
            message: normalize_message(message),
            context: dx_digest::blake3(context),
            count: 1,
        }
    }

    fn commit_baseline(harness: &Harness, entries: Vec<BaselineEntry>) {
        harness.write_source("dx.toml", &format!("[quality]\nbaseline = \"{REL}\"\n"));
        let rendered = render_baseline(&BaselineFile { entries }).expect("render");
        harness.write_source(REL, &rendered);
    }

    fn read_baseline(harness: &Harness) -> BaselineFile {
        let bytes = std::fs::read(harness.workspace.join(REL)).expect("baseline bytes");
        parse_baseline(&bytes).expect("baseline parses")
    }

    fn event<'a>(events: &'a [serde_json::Value], kind: &str) -> &'a serde_json::Value {
        events
            .iter()
            .find(|event| event["event"] == serde_json::json!(kind))
            .unwrap_or_else(|| panic!("{kind} event must exist"))
    }

    #[test]
    fn recorded_finding_passes_check_while_new_finding_fails() {
        let mut harness = Harness::new("baseline-suppress");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0);
        assert!(out.contains("unused [lint-tool/rule] (baselined)"), "{out}");
        assert!(
            out.contains("Baseline quality/baseline.json: 1 total, 0 new, 1 suppressed."),
            "{out}"
        );
        let mut harness = Harness::new("baseline-new");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![
                    Harness::diagnostic("unused", false),
                    Harness::diagnostic("brand new", false),
                ],
                vec![],
            ),
        );
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(
            out.contains("Baseline quality/baseline.json: 2 total, 1 new, 1 suppressed."),
            "{out}"
        );
        assert!(
            out.contains("brand new [lint-tool/rule]")
                && !out.contains("brand new [lint-tool/rule] (baselined)"),
            "{out}"
        );
    }

    #[test]
    fn moved_line_keeps_suppression() {
        let mut harness = Harness::new("baseline-moved");
        harness.write_source(PATH, "a = 1\nx = 1\n");
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        let mut shifted = Harness::diagnostic("unused", false);
        shifted.start_byte = Some(6);
        shifted.end_byte = Some(7);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![shifted], vec![]),
        );
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("(baselined)"), "{out}");
    }

    #[test]
    fn stale_entry_fails_check_and_apply_prunes() {
        let mut harness = Harness::new("baseline-stale");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(
            err.contains("Baseline stale: src/a.py [lint-tool/lint-tool/rule]: unused (run with --apply to refresh)."),
            "{err}"
        );
        let (code, out, _) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Baseline quality/baseline.json refreshed."),
            "{out}"
        );
        assert!(read_baseline(&harness).entries.is_empty());
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{err}");
    }

    #[test]
    fn missing_file_fails_check_but_apply_creates() {
        let mut harness = Harness::new("baseline-missing");
        harness.write_source(PATH, BODY);
        harness.write_source("dx.toml", &format!("[quality]\nbaseline = \"{REL}\"\n"));
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(
            err.contains("dx: verification_failed: baseline quality/baseline.json is missing"),
            "{err}"
        );
        let (code, out, _) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(read_baseline(&harness).entries.is_empty());
    }

    #[test]
    fn malformed_baseline_fails_both_modes() {
        for (name, mode) in [
            ("malformed-check", "--check"),
            ("malformed-apply", "--apply"),
        ] {
            let mut harness = Harness::new(name);
            harness.write_source(PATH, BODY);
            harness.write_source("dx.toml", &format!("[quality]\nbaseline = \"{REL}\"\n"));
            harness.write_source(REL, "{not json");
            harness.results.insert(
                "//test:corpus".to_owned(),
                harness.valid_result(vec![], vec![]),
            );
            let (code, _, err) = harness.run(&["lint", mode, "--output=text"]);
            assert_eq!(code, 1, "{name}");
            assert!(
                err.contains("verification_failed") && err.contains("malformed baseline"),
                "{name}: {err}"
            );
            assert_eq!(
                std::fs::read(harness.workspace.join(REL)).expect("bytes"),
                b"{not json",
                "{name} never rewrites a malformed file"
            );
        }
    }

    #[test]
    fn incomplete_collection_never_passes_through_a_baseline() {
        let mut harness = Harness::new("baseline-incomplete");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        harness.fail_target = true;
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(!err.contains("Baseline stale"), "{err}");
    }

    #[test]
    fn json_events_mark_suppressed_and_carry_counts() {
        let mut harness = Harness::new("baseline-json");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![
                    Harness::diagnostic("unused", false),
                    Harness::diagnostic("brand new", false),
                ],
                vec![],
            ),
        );
        let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
        assert_eq!(code, 1);
        let events = json_events(&out);
        let diagnostics: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == serde_json::json!("diagnostic"))
            .collect();
        assert_eq!(diagnostics.len(), 2);
        let held = diagnostics
            .iter()
            .find(|event| event["message"] == serde_json::json!("unused"))
            .expect("recorded");
        assert_eq!(held["baseline"], serde_json::json!("suppressed"));
        let fresh = diagnostics
            .iter()
            .find(|event| event["message"] == serde_json::json!("brand new"))
            .expect("new");
        assert!(fresh.get("baseline").is_none());
        let finished = event(&events, "command_finished");
        assert_eq!(
            finished["baseline"],
            serde_json::json!({"total": 2, "new": 1, "suppressed": 1})
        );
    }

    #[test]
    fn sarif_report_carries_baseline_counts() {
        let mut harness = Harness::new("baseline-sarif");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, _, _) = harness.run(&[
            "lint",
            "--check",
            "--output=text",
            "--report=sarif=out.sarif",
        ]);
        assert_eq!(code, 0);
        let text = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
        let document: serde_json::Value = serde_json::from_str(&text).expect("json");
        let runs = document["runs"].as_array().expect("runs");
        assert_eq!(
            runs[0]["properties"]["dxBaseline"],
            serde_json::json!({
                "file": "quality/baseline.json",
                "total": 1,
                "new": 0,
                "suppressed": 1,
            })
        );
        assert_eq!(runs[0]["results"].as_array().expect("results").len(), 1);
    }

    #[test]
    fn apply_refresh_adds_new_and_preserves_out_of_scope() {
        let mut harness = Harness::new("baseline-refresh");
        harness.write_source(PATH, BODY);
        let mut foreign = recorded("old", b"keep");
        foreign.tool = "other-tool".to_owned();
        foreign.path = "src/keep.py".to_owned();
        commit_baseline(&harness, vec![recorded("unused", LINE), foreign]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![
                    Harness::diagnostic("unused", false),
                    Harness::diagnostic("brand new", false),
                ],
                vec![],
            ),
        );
        let (code, out, _) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Baseline quality/baseline.json refreshed."),
            "{out}"
        );
        let next = read_baseline(&harness);
        assert_eq!(next.entries.len(), 3);
        assert!(next.entries.iter().any(|entry| entry.tool == "other-tool"));
        assert!(next
            .entries
            .iter()
            .any(|entry| entry.message == "brand new"));
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}");
    }

    #[test]
    fn rejected_selection_fails_before_analysis() {
        let mut harness = Harness::new("baseline-selection");
        harness.write_source(PATH, BODY);
        harness.write_source("dx.toml", "[quality]\nbaseline = \"/abs.json\"\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_ne!(code, 0);
        assert!(err.contains("baseline"), "{err}");
    }

    #[test]
    fn unconfigured_runs_ignore_baselines() {
        let mut harness = Harness::new("baseline-absent");
        harness.write_source(PATH, BODY);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(!out.contains("Baseline "), "{out}");
        assert!(!out.contains("(baselined)"), "{out}");
    }

    #[test]
    fn ci_ignores_the_local_baseline_selection() {
        let mut harness = Harness::new("baseline-ci-local");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.write_source("dx.local.toml", "[quality]\nbaseline = \"local.json\"\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let words: &[&str] = &["lint", "--check", "--output=text"];
        let (code, out, _) = harness.run_with_ci(words, true);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Baseline quality/baseline.json"), "{out}");
        let (code, out, err) = harness.run_with_ci(words, false);
        assert_ne!(code, 0);
        assert!(format!("{out}{err}").contains("local.json"), "{out}{err}");
        harness.write_source("dx.local.toml", "[broken");
        let (code, out, _) = harness.run_with_ci(words, true);
        assert_eq!(code, 0, "{out}");
        let (code, _, err) = harness.run_with_ci(words, false);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("dx.local.toml"), "{err}");
    }

    #[test]
    fn threshold_still_applies_to_new_findings_only() {
        let mut harness = Harness::new("baseline-threshold");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, _, _) = harness.run(&["lint", "--check", "--output=text", "--fail-on=error"]);
        assert_eq!(code, 0);
        let view = apply_baseline(
            &harness.workspace,
            &[],
            &AnalyzedScope::default(),
            &BTreeMap::new(),
            BaselineGate {
                complete: true,
                stable: true,
                applied_all: true,
            },
            false,
            Some(REL),
        )
        .expect("applies")
        .expect("configured");
        assert_eq!(view.counts(), [0, 0, 0]);
    }

    fn producer_result(
        harness: &Harness,
        producer: &str,
        tool: &str,
        path: &str,
        messages: &[&str],
    ) -> Vec<u8> {
        let bytes = std::fs::read(harness.workspace.join(path)).expect("source");
        let snapshot = vec![FileSnapshot {
            path: path.to_owned(),
            digest: dx_digest::blake3(&bytes).to_vec(),
        }];
        let diagnostics = messages
            .iter()
            .map(|message| Harness::diagnostic_with(2, tool, path, message, false))
            .collect();
        encode_validated(&QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: producer.to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: tool.to_owned(),
                class_ids: vec!["python".to_owned()],
                source_paths: vec![path.to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: snapshot.clone(),
            terminal_snapshot: snapshot,
            initial_diagnostics: diagnostics,
            terminal_diagnostics: vec![],
            replacements: vec![],
        })
        .expect("encode")
    }

    fn entry_for(tool: &str, path: &str, message: &str, context: &[u8]) -> BaselineEntry {
        let mut entry = recorded(message, context);
        entry.tool = tool.to_owned();
        entry.rule = format!("{tool}/rule");
        entry.path = path.to_owned();
        entry
    }

    fn two_producers(name: &str) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source("src/a.py", "a = 1\n");
        harness.write_source("src/b.py", "b = 1\n");
        commit_baseline(
            &harness,
            vec![
                entry_for("A", "src/a.py", "gone", b"a = 1"),
                entry_for("A", "src/b.py", "keep", b"b = 1"),
                entry_for("B", "src/b.py", "held", b"b = 1"),
            ],
        );
        let a = producer_result(&harness, "//test:a", "A", "src/a.py", &[]);
        let b = producer_result(&harness, "//test:b", "B", "src/b.py", &["held"]);
        harness.results.insert("//test:a".to_owned(), a);
        harness.results.insert("//test:b".to_owned(), b);
        harness
    }

    #[test]
    fn disjoint_producers_do_not_claim_each_others_files() {
        let harness = two_producers("baseline-disjoint-check");
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("src/a.py [A/A/rule]: gone"), "{err}");
        assert!(!err.contains("keep"), "{err}");
        assert!(
            out.contains("Baseline quality/baseline.json: 1 total, 0 new, 1 suppressed."),
            "{out}"
        );
        let harness = two_producers("baseline-disjoint-json");
        let (_, out, _) = harness.run(&["lint", "--check", "--output=json"]);
        let events = json_events(&out);
        assert_eq!(
            event(&events, "command_finished")["baseline"],
            serde_json::json!({"total": 1, "new": 0, "suppressed": 1})
        );
    }

    #[test]
    fn disjoint_producers_refresh_only_the_proven_pairs() {
        let harness = two_producers("baseline-disjoint-apply");
        let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        let next = read_baseline(&harness);
        let mut kept: Vec<(String, String, String)> = next
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.tool.clone(),
                    entry.path.clone(),
                    entry.message.clone(),
                )
            })
            .collect();
        kept.sort();
        assert_eq!(
            kept,
            vec![
                ("A".to_owned(), "src/b.py".to_owned(), "keep".to_owned()),
                ("B".to_owned(), "src/b.py".to_owned(), "held".to_owned()),
            ]
        );
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{err}");
    }

    #[test]
    fn disjoint_producers_report_coverage_in_sarif() {
        let harness = two_producers("baseline-disjoint-sarif");
        let (_, _, _) = harness.run(&[
            "lint",
            "--check",
            "--output=text",
            "--report=sarif=out.sarif",
        ]);
        let text = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
        let document: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(
            document["runs"][0]["properties"]["dxBaseline"],
            serde_json::json!({
                "file": "quality/baseline.json",
                "total": 1,
                "new": 0,
                "suppressed": 1,
            })
        );
    }

    fn pathless_result(
        harness: &Harness,
        producer: &str,
        tool: &str,
        messages: &[&str],
    ) -> Vec<u8> {
        let bytes = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
        let snapshot = vec![FileSnapshot {
            path: "src/a.py".to_owned(),
            digest: dx_digest::blake3(&bytes).to_vec(),
        }];
        let diagnostics = messages
            .iter()
            .map(|message| {
                let mut diagnostic = Harness::diagnostic_with(2, tool, "", message, false);
                diagnostic.start_byte = None;
                diagnostic.end_byte = None;
                diagnostic
            })
            .collect();
        encode_validated(&QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: producer.to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: tool.to_owned(),
                class_ids: vec!["python".to_owned()],
                source_paths: vec!["src/a.py".to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: snapshot.clone(),
            terminal_snapshot: snapshot,
            initial_diagnostics: diagnostics,
            terminal_diagnostics: vec![],
            replacements: vec![],
        })
        .expect("encode")
    }

    fn pathless_entry(tool: &str, target: &str, message: &str) -> BaselineEntry {
        let mut entry = entry_for(tool, "", message, b"");
        entry.target = target.to_owned();
        entry
    }

    fn pathless_producers(name: &str, labels: &[&str]) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source("src/a.py", "a = 1\n");
        commit_baseline(
            &harness,
            vec![
                pathless_entry("A", "//test:a", "dup"),
                pathless_entry("A", "//test:b", "dup"),
                pathless_entry("A", "//test:z", "old"),
            ],
        );
        for label in labels {
            let bytes = pathless_result(&harness, label, "A", &["dup"]);
            harness.results.insert((*label).to_owned(), bytes);
        }
        harness
    }

    fn pathless_kept(harness: &Harness) -> Vec<(String, String)> {
        let mut kept: Vec<(String, String)> = read_baseline(harness)
            .entries
            .iter()
            .map(|entry| (entry.target.clone(), entry.message.clone()))
            .collect();
        kept.sort();
        kept
    }

    #[test]
    fn pathless_findings_are_scoped_by_the_target_that_produced_them() {
        let harness = pathless_producers("baseline-pathless-check", &["//test:a", "//test:b"]);
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!err.contains("Baseline stale"), "{err}");
        assert!(
            out.contains("Baseline quality/baseline.json: 2 total, 0 new, 2 suppressed."),
            "{out}"
        );
        let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            pathless_kept(&harness),
            vec![
                ("//test:a".to_owned(), "dup".to_owned()),
                ("//test:b".to_owned(), "dup".to_owned()),
                ("//test:z".to_owned(), "old".to_owned()),
            ]
        );
    }

    #[test]
    fn a_narrower_run_keeps_the_other_targets_pathless_entries() {
        let harness = pathless_producers("baseline-pathless-narrow", &["//test:b"]);
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!err.contains("Baseline stale"), "{err}");
        assert!(
            out.contains("Baseline quality/baseline.json: 1 total, 0 new, 1 suppressed."),
            "{out}"
        );
        let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(pathless_kept(&harness).len(), 3);
    }

    #[test]
    fn a_pathless_entry_goes_stale_when_its_target_ran_clean() {
        let mut harness = pathless_producers("baseline-pathless-stale", &["//test:b"]);
        let clean = pathless_result(&harness, "//test:a", "A", &[]);
        harness.results.insert("//test:a".to_owned(), clean);
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(err.contains("//test:a [A/A/rule]: dup"), "{err}");
        let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            pathless_kept(&harness),
            vec![
                ("//test:b".to_owned(), "dup".to_owned()),
                ("//test:z".to_owned(), "old".to_owned()),
            ]
        );
    }

    fn assert_apply_leaves_baseline(harness: &Harness) {
        let before = std::fs::read(harness.workspace.join(REL)).ok();
        let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(!out.contains("refreshed"), "{out}");
        assert_eq!(std::fs::read(harness.workspace.join(REL)).ok(), before);
    }

    #[test]
    fn incomplete_apply_preserves_baseline_bytes() {
        let mut failed = two_producers("baseline-partial-failed");
        failed.failed_labels.push("//test:a".to_owned());
        assert_apply_leaves_baseline(&failed);
        let (code, out, err) = failed.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!err.contains("Baseline stale"), "{err}");
        assert!(
            out.contains("Baseline quality/baseline.json: 1 total, 0 new, 1 suppressed."),
            "{out}"
        );

        let mut malformed = two_producers("baseline-partial-malformed");
        malformed
            .results
            .insert("//test:a".to_owned(), b"not a result".to_vec());
        assert_apply_leaves_baseline(&malformed);

        let mut missing = two_producers("baseline-partial-missing");
        missing.bazel_code = 1;
        assert_apply_leaves_baseline(&missing);
    }

    #[test]
    fn incomplete_apply_never_creates_a_baseline() {
        let mut harness = two_producers("baseline-partial-create");
        std::fs::remove_file(harness.workspace.join(REL)).expect("remove");
        harness.failed_labels.push("//test:a".to_owned());
        let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 1);
        assert!(err.contains("analysis was incomplete"), "{err}");
        assert!(!harness.workspace.join(REL).exists());
    }

    fn refused_fix(name: &str) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![Harness::diagnostic("unused", false)],
                vec![harness.replacement(b"y")],
            ),
        );
        harness
    }

    fn failing_source(harness: &Harness) -> FailingWrites {
        FailingWrites::new(&[harness.workspace.join(PATH)])
    }

    #[test]
    fn refused_fix_fails_and_keeps_baseline_bytes() {
        let harness = refused_fix("baseline-refused");
        let before = std::fs::read(harness.workspace.join(REL)).expect("bytes");
        let (code, out, err) = harness.run_with_fs(
            &["lint", "--apply", "--output=text"],
            &failing_source(&harness),
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("unused [lint-tool/rule] (baselined)"), "{out}");
        assert!(
            err.contains("Not applied: src/a.py (write_failed)"),
            "{err}"
        );
        assert!(!out.contains("refreshed"), "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join(REL)).expect("bytes"),
            before
        );
        assert_eq!(
            std::fs::read(harness.workspace.join(PATH)).expect("source"),
            BODY.as_bytes()
        );

        let pruned = refused_fix("baseline-refused-prune");
        commit_baseline(
            &pruned,
            vec![recorded("unused", LINE), recorded("gone", LINE)],
        );
        let before = std::fs::read(pruned.workspace.join(REL)).expect("bytes");
        let (code, out, err) = pruned.run_with_fs(
            &["lint", "--apply", "--output=text"],
            &failing_source(&pruned),
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("refreshed"), "{out}");
        assert_eq!(
            std::fs::read(pruned.workspace.join(REL)).expect("bytes"),
            before
        );
    }

    #[test]
    fn refused_fix_never_creates_a_baseline() {
        let harness = refused_fix("baseline-refused-create");
        std::fs::remove_file(harness.workspace.join(REL)).expect("remove");
        let (code, out, err) = harness.run_with_fs(
            &["lint", "--apply", "--output=text"],
            &failing_source(&harness),
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("baseline quality/baseline.json is missing and was not recorded: a fix was not applied"),
            "{err}"
        );
        assert!(!harness.workspace.join(REL).exists());
    }

    #[test]
    fn baseline_gate_names_the_first_blocking_cause() {
        let gate = |complete, stable, applied_all| BaselineGate {
            complete,
            stable,
            applied_all,
        };
        assert_eq!(gate(true, true, true).refresh_blocked(), None);
        assert_eq!(
            gate(false, false, false).refresh_blocked(),
            Some("analysis was incomplete")
        );
        assert_eq!(
            gate(true, false, false).refresh_blocked(),
            Some("a result did not converge")
        );
        assert_eq!(
            gate(true, true, false).refresh_blocked(),
            Some("a fix was not applied")
        );
        assert!(gate(true, true, false).matching_complete());
    }

    #[test]
    fn unidentified_source_refuses_refresh() {
        let mut harness = Harness::new("baseline-identity");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        let mut diagnostic = Harness::diagnostic("unused", false);
        diagnostic.start_byte = Some(900);
        diagnostic.end_byte = Some(901);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![diagnostic], vec![]),
        );
        let before = std::fs::read(harness.workspace.join(REL)).expect("bytes");
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(!err.contains("Baseline stale"), "{err}");
        let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 1);
        assert!(err.contains("source identity is unavailable"), "{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join(REL)).expect("bytes"),
            before
        );
    }

    #[test]
    fn crlf_and_unicode_lines_keep_their_identity() {
        let mut harness = Harness::new("baseline-crlf");
        harness.write_source(PATH, "h = \"caf\u{e9}\"\r\nx = 1\r\n");
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        let mut moved = Harness::diagnostic("unused", false);
        moved.start_byte = Some(14);
        moved.end_byte = Some(15);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![moved], vec![]),
        );
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("(baselined)"), "{out}");
    }

    #[test]
    fn duplicate_findings_respect_recorded_counts() {
        let mut harness = Harness::new("baseline-duplicates");
        harness.write_source(PATH, BODY);
        commit_baseline(&harness, vec![recorded("unused", LINE)]);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![
                    Harness::diagnostic("unused", false),
                    Harness::diagnostic("unused", false),
                ],
                vec![],
            ),
        );
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(
            out.contains("Baseline quality/baseline.json: 2 total, 1 new, 1 suppressed."),
            "{out}"
        );
    }
}
