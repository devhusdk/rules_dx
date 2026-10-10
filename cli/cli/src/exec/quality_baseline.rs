use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_apply::{FileSystem, RealFileSystem};
use dx_fingerprint::baseline::{
    describe_entry, line_context_digest, match_baseline, normalize_message, parse_baseline,
    refresh_baseline, render_baseline, AnalyzedScope, BaselineFile, Finding,
};
use dx_output::DiagnosticEvent;

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

fn finding_for(diagnostic: &DiagnosticEvent, files: &BTreeMap<String, Option<Vec<u8>>>) -> Finding {
    let context = match (&diagnostic.path, diagnostic.range) {
        (None, _) => Some(dx_digest::blake3(b"")),
        (Some(path), None) => files
            .get(path)
            .and_then(|bytes| bytes.as_ref())
            .map(|bytes| dx_digest::blake3(bytes)),
        (Some(path), Some((start, _))) => files
            .get(path)
            .and_then(|bytes| bytes.as_ref())
            .and_then(|bytes| line_context_digest(bytes, start)),
    };
    Finding {
        tool: diagnostic.tool.clone(),
        rule: diagnostic.rule.clone().unwrap_or_default(),
        path: diagnostic.path.clone().unwrap_or_default(),
        message: normalize_message(&diagnostic.message),
        context,
    }
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

fn analyzed_scope(
    tools: &[String],
    terminal_digests: &BTreeMap<String, [u8; 32]>,
) -> AnalyzedScope {
    AnalyzedScope {
        tools: tools.iter().cloned().collect(),
        paths: terminal_digests.keys().cloned().collect(),
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

pub(crate) fn apply_baseline(
    workspace: &Path,
    status: &[DiagnosticEvent],
    tools: &[String],
    terminal_digests: &BTreeMap<String, [u8; 32]>,
    complete: bool,
    apply: bool,
) -> Result<Option<BaselineView>, (String, String)> {
    let fail = |message: String| (CODE_VERIFICATION_FAILED.to_owned(), message);
    let (defaults, _) =
        dx_adopt::load_defaults(workspace).map_err(|error| fail(error.to_string()))?;
    let Some(rel) = defaults.baseline else {
        return Ok(None);
    };
    let full = dx_adopt::join_quality_baseline(workspace, &rel);
    let bytes = match std::fs::read(&full) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(fail(format!("cannot read baseline {rel}: {error}"))),
    };
    let scope = analyzed_scope(tools, terminal_digests);
    let files = read_sources(workspace, status);
    let findings: Vec<Finding> = status
        .iter()
        .map(|diagnostic| finding_for(diagnostic, &files))
        .collect();
    let Some(bytes) = bytes else {
        if !apply {
            return Err(fail(format!(
                "baseline {rel} is missing: run with --apply to record current findings"
            )));
        }
        let next = refresh_baseline(&BaselineFile::default(), &findings, &scope);
        let rendered = render_baseline(&next).map_err(|error| fail(error.to_string()))?;
        write_baseline(&full, rendered.as_bytes())
            .map_err(|error| fail(format!("cannot write baseline {rel}: {error}")))?;
        return Ok(Some(view_for(
            &rel, &next, &findings, &scope, complete, true,
        )));
    };
    let baseline = parse_baseline(&bytes).map_err(|error| fail(format!("{rel}: {error}")))?;
    if !apply {
        return Ok(Some(view_for(
            &rel, &baseline, &findings, &scope, complete, false,
        )));
    }
    let next = refresh_baseline(&baseline, &findings, &scope);
    let rendered = render_baseline(&next).map_err(|error| fail(error.to_string()))?;
    write_baseline(&full, rendered.as_bytes())
        .map_err(|error| fail(format!("cannot write baseline {rel}: {error}")))?;
    Ok(Some(view_for(
        &rel, &next, &findings, &scope, complete, true,
    )))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{json_events, Harness};
    use super::*;
    use dx_fingerprint::baseline::{
        normalize_message, render_baseline, BaselineEntry, BaselineFile,
    };
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
        assert!(
            out.contains("unused [lint-tool/lint-tool/rule] (baselined)"),
            "{out}"
        );
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
            out.contains("brand new [lint-tool/lint-tool/rule]")
                && !out.contains("brand new [lint-tool/lint-tool/rule] (baselined)"),
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
        let view = apply_baseline(&harness.workspace, &[], &[], &BTreeMap::new(), true, false)
            .expect("applies")
            .expect("configured");
        assert_eq!(view.counts(), [0, 0, 0]);
    }
}
