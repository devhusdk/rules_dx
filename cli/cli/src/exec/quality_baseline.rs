use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use dx_apply::{FileSystem, RealFileSystem};
use dx_fingerprint::baseline::{
    context_line, fingerprint, match_baseline, parse_baseline, render_baseline, stale_entries,
    BaselineScope, Fingerprint,
};
use dx_output::DiagnosticEvent;

pub(crate) const CODE_BASELINE_INVALID: &str = "baseline_invalid";
pub(crate) const CODE_BASELINE_STALE: &str = "baseline_stale";
pub(crate) const CODE_BASELINE_REFRESH_FAILED: &str = "baseline_refresh_failed";

pub(crate) const SUPPRESSED_MARK: &str = " (suppressed)";

fn check_rel(rel: &str) -> Result<(), String> {
    if rel.is_empty() {
        return Err("baseline path is empty: want a workspace-relative file".to_owned());
    }
    if rel.contains('\\') {
        return Err(format!(
            "baseline path {rel:?} uses a backslash: want forward slashes"
        ));
    }
    if rel.contains("//") || rel.ends_with('/') {
        return Err(format!(
            "baseline path {rel:?} has an empty component: want a file path"
        ));
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(format!(
            "baseline path {rel:?} is absolute: want a workspace-relative file"
        ));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!(
                "baseline path {rel:?} escapes the workspace: want a file below the root"
            ));
        }
    }
    Ok(())
}

pub(crate) enum BaselineInput {
    Absent,
    Entries(Vec<Fingerprint>),
}

pub(crate) fn load_baseline(
    workspace: &Path,
    rel: &str,
) -> Result<BaselineInput, String> {
    check_rel(rel)?;
    let bytes = match std::fs::read(workspace.join(rel)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(BaselineInput::Absent)
        }
        Err(error) => return Err(format!("cannot read baseline {rel}: {error}")),
        Ok(bytes) => bytes,
    };
    parse_baseline(&bytes)
        .map(BaselineInput::Entries)
        .map_err(|error| format!("invalid baseline {rel}: {error}"))
}

fn fingerprint_diagnostic(
    workspace: &Path,
    diagnostic: &DiagnosticEvent,
) -> Option<Fingerprint> {
    let path = diagnostic.path.as_ref()?;
    let bytes = std::fs::read(workspace.join(path)).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let context = match diagnostic.range {
        Some((start, _)) => context_line(text, start)?,
        None => "",
    };
    Some(fingerprint(
        &diagnostic.tool,
        diagnostic.rule.as_deref().unwrap_or(""),
        path,
        &diagnostic.message,
        Some(context),
    ))
}

pub(crate) struct BaselineOutcome {
    pub(crate) suppressed: Vec<bool>,
    pub(crate) stale_indices: Vec<usize>,
    pub(crate) new_fingerprints: Vec<Fingerprint>,
    pub(crate) total: usize,
    pub(crate) suppressed_count: usize,
    pub(crate) new_count: usize,
}

pub(crate) fn apply_baseline(
    workspace: &Path,
    status: &[DiagnosticEvent],
    analyzed: &BTreeSet<(String, String)>,
    complete: bool,
    entries: &[Fingerprint],
) -> BaselineOutcome {
    let matchable: Vec<(usize, Fingerprint)> = status
        .iter()
        .enumerate()
        .filter_map(|(index, diagnostic)| {
            fingerprint_diagnostic(workspace, diagnostic).map(|finding| (index, finding))
        })
        .collect();
    let current: Vec<Fingerprint> = matchable
        .iter()
        .map(|(_, finding)| finding.clone())
        .collect();
    let matched = match_baseline(&current, entries);
    let mut suppressed = vec![false; status.len()];
    let mut new_fingerprints = Vec::new();
    for ((index, finding), flag) in matchable.iter().zip(matched.suppressed.iter()) {
        suppressed[*index] = *flag;
        if !flag {
            new_fingerprints.push(finding.clone());
        }
    }
    let existing: BTreeSet<String> = entries
        .iter()
        .map(|entry| entry.path.clone())
        .filter(|path| workspace.join(path).is_file())
        .collect();
    let scope = BaselineScope {
        complete,
        analyzed: analyzed.clone(),
        existing,
    };
    let stale_indices = stale_entries(entries, &matched.remaining, &scope);
    let suppressed_count = suppressed.iter().filter(|flag| **flag).count();
    BaselineOutcome {
        suppressed,
        stale_indices,
        new_fingerprints,
        total: status.len(),
        suppressed_count,
        new_count: status.len() - suppressed_count,
    }
}

pub(crate) struct BaselineRefresh {
    pub(crate) pruned: usize,
    pub(crate) added: usize,
    pub(crate) bytes: String,
}

pub(crate) fn refresh_entries(
    entries: &[Fingerprint],
    stale_indices: &[usize],
    new_fingerprints: &[Fingerprint],
) -> Result<BaselineRefresh, String> {
    let stale: BTreeSet<usize> = stale_indices.iter().copied().collect();
    let mut kept = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if !stale.contains(&index) {
            kept.push(entry.clone());
        }
    }
    let added = new_fingerprints.len();
    kept.extend(new_fingerprints.iter().cloned());
    let bytes = render_baseline(&kept).map_err(|error| error.to_string())?;
    Ok(BaselineRefresh {
        pruned: stale.len(),
        added,
        bytes,
    })
}

pub(crate) fn write_refresh(
    workspace: &Path,
    rel: &str,
    bytes: &str,
) -> Result<(), String> {
    let target: PathBuf = workspace.join(rel);
    if target.is_dir() {
        return Err(format!("baseline {rel} is a directory"));
    }
    match target.parent() {
        Some(parent) if !parent.is_dir() => {
            return Err(format!(
                "parent directory of baseline {rel} does not exist"
            ))
        }
        _ => {}
    }
    RealFileSystem
        .write_atomic(&target, bytes.as_bytes())
        .map_err(|error| format!("cannot write baseline {rel}: {error}"))
}

pub(crate) struct BaselineReport {
    pub(crate) rel: String,
    pub(crate) total: usize,
    pub(crate) suppressed_count: usize,
    pub(crate) new_count: usize,
    pub(crate) refreshed: Option<(usize, usize)>,
    pub(crate) stale: Vec<Fingerprint>,
}

pub(crate) fn summary_message(report: &BaselineReport) -> String {
    format!(
        "Baseline {}: {} total, {} suppressed, {} new.",
        report.rel, report.total, report.suppressed_count, report.new_count
    )
}

pub(crate) fn refreshed_message(report: &BaselineReport) -> Option<String> {
    report
        .refreshed
        .map(|(pruned, added)| {
            format!(
                "Refreshed baseline {}: pruned {pruned} stale, added {added} new.",
                report.rel
            )
        })
}

pub(crate) fn notice_message(report: &BaselineReport) -> String {
    let mut message = format!(
        "baseline {}: total={} suppressed={} new={}",
        report.rel, report.total, report.suppressed_count, report.new_count
    );
    if let Some((pruned, added)) = report.refreshed {
        message.push_str(&format!(" pruned={pruned} added={added}"));
    }
    message
}

pub(crate) fn stale_line(entry: &Fingerprint) -> String {
    let rule = if entry.rule.is_empty() {
        "-"
    } else {
        entry.rule.as_str()
    };
    format!(
        "Stale baseline entry {} {rule} {}: {}",
        entry.tool, entry.path, entry.message
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use dx_fingerprint::baseline::{fingerprint, render_baseline, Fingerprint};
    use quality_result::proto;
    use quality_result::proto::FileSnapshot;

    fn entry_for(message: &str, context: &str) -> Fingerprint {
        fingerprint(
            "lint-tool",
            "lint-tool/rule",
            "src/a.py",
            message,
            Some(context),
        )
    }

    fn baseline_harness(name: &str, entries: &[Fingerprint]) -> Harness {
        let harness = Harness::new(name);
        harness.write_source("dx.toml", "[dx]\nbaseline = \"quality-baseline.json\"\n");
        let rendered = render_baseline(entries).expect("baseline renders");
        harness.write_source("quality-baseline.json", &rendered);
        harness
    }

    fn run_with_defaults(harness: &Harness, words: &[&str]) -> (i32, String, String) {
        let (defaults, _) =
            dx_adopt::defaults::load_defaults(&harness.workspace).expect("defaults load");
        let owned = crate::test_support::strings(words);
        let env_get = |_: &str| -> Option<String> { None };
        let parsed =
            crate::args::parse_with(&owned, &env_get, &defaults).expect("parse with defaults");
        let invocation =
            crate::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("here");
        let runner = harness.runner();
        harness.execute_with(&invocation, &runner)
    }

    fn baseline_bytes(harness: &Harness) -> Vec<u8> {
        std::fs::read(harness.workspace.join("quality-baseline.json")).expect("baseline bytes")
    }

    #[test]
    fn suppressed_finding_passes_without_touching_baseline_bytes() {
        let mut harness = baseline_harness("baseline-suppressed", &[entry_for("unused", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let before = baseline_bytes(&harness);
        let (code, out, err) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 0);
        assert!(out.contains("warning src/a.py: unused [lint-tool/lint-tool/rule] (suppressed)"));
        assert!(out.contains("Baseline quality-baseline.json: 1 total, 1 suppressed, 0 new."));
        assert_eq!(err, "");
        assert_eq!(baseline_bytes(&harness), before);
    }

    #[test]
    fn new_finding_still_fails() {
        let mut harness = baseline_harness("baseline-new", &[entry_for("old news", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(out.contains("warning src/a.py: unused [lint-tool/lint-tool/rule]"));
        assert!(!out.contains("(suppressed)"));
        assert!(out.contains("Baseline quality-baseline.json: 1 total, 0 suppressed, 1 new."));
    }

    #[test]
    fn moved_line_matches_baseline() {
        let mut harness = baseline_harness("baseline-moved", &[entry_for("unused", "x = 1")]);
        harness.write_source("src/a.py", "head = 1\nx = 1\n");
        let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
        let moved = proto::Diagnostic {
            severity: proto::Severity::Warning as i32,
            message: "unused".to_owned(),
            tool_id: "lint-tool".to_owned(),
            rule_id: "lint-tool/rule".to_owned(),
            path: "src/a.py".to_owned(),
            start_byte: Some(9),
            end_byte: Some(10),
            fixable: false,
            ..Default::default()
        };
        let bytes = harness.result_full(
            vec![moved],
            vec![],
            vec![],
            vec![FileSnapshot {
                path: "src/a.py".to_owned(),
                digest: dx_digest::blake3(&original).to_vec(),
            }],
        );
        harness.results.insert("//test:corpus".to_owned(), bytes);
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 0);
        assert!(out.contains("(suppressed)"));
    }

    #[test]
    fn renamed_path_needs_refresh() {
        let mut harness = baseline_harness("baseline-renamed", &[entry_for("unused", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.write_source("src/b.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![Harness::diagnostic_with(
                    proto::Severity::Warning as i32,
                    "lint-tool",
                    "src/b.py",
                    "unused",
                    false,
                )],
                vec![],
            ),
        );
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(out.contains("Baseline quality-baseline.json: 1 total, 0 suppressed, 1 new."));
    }

    #[test]
    fn stale_entry_fails_the_run() {
        let mut harness = baseline_harness("baseline-stale", &[entry_for("fixed", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        let (code, out, err) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(out.contains("Baseline quality-baseline.json: 0 total, 0 suppressed, 0 new."));
        assert!(out.contains("Stale baseline entry lint-tool lint-tool/rule src/a.py: fixed"));
        assert!(err.contains("dx: baseline_stale: quality-baseline.json has 1 stale entries"));
    }

    #[test]
    fn malformed_baseline_fails_but_shows_findings() {
        let mut harness = Harness::new("baseline-malformed");
        harness.write_source("dx.toml", "[dx]\nbaseline = \"quality-baseline.json\"\n");
        harness.write_source("quality-baseline.json", "{not json");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, out, err) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(out.contains("warning src/a.py: unused [lint-tool/lint-tool/rule]"));
        assert!(err.contains("dx: baseline_invalid: invalid baseline quality-baseline.json:"));
    }

    #[test]
    fn missing_baseline_file_reads_as_empty_in_check_mode() {
        let mut harness = Harness::new("baseline-absent");
        harness.write_source("dx.toml", "[dx]\nbaseline = \"quality-baseline.json\"\n");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(out.contains("Baseline quality-baseline.json: 1 total, 0 suppressed, 1 new."));
        assert!(!harness.workspace.join("quality-baseline.json").exists());
    }

    #[test]
    fn apply_refresh_prunes_stale_and_adds_new() {
        let mut harness = baseline_harness(
            "baseline-refresh",
            &[entry_for("fixed", "x = 1"), entry_for("kept", "x = 1")],
        );
        harness.write_source("src/a.py", "x = 1\n");
        let current = vec![
            Harness::diagnostic("kept", false),
            Harness::diagnostic("fresh", false),
        ];
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(current, vec![]),
        );
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--apply", "--output=text"]);
        assert_eq!(code, 1);
        assert!(out.contains("Baseline quality-baseline.json: 2 total, 1 suppressed, 1 new."));
        assert!(out.contains(
            "Refreshed baseline quality-baseline.json: pruned 1 stale, added 1 new."
        ));
        let refreshed = baseline_bytes(&harness);
        let entries =
            dx_fingerprint::baseline::parse_baseline(&refreshed).expect("refreshed parses");
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&entry_for("kept", "x = 1")));
        assert!(entries.contains(&entry_for("fresh", "x = 1")));
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 0);
        assert!(out.contains("Baseline quality-baseline.json: 2 total, 2 suppressed, 0 new."));
    }

    #[test]
    fn incomplete_collection_cannot_report_stale() {
        let mut harness = baseline_harness("baseline-incomplete", &[entry_for("fixed", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness.fail_target = true;
        let (code, out, err) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(!out.contains("Stale baseline entry"));
        assert!(!err.contains("baseline_stale"));
        assert!(out.contains("Baseline quality-baseline.json: 0 total, 0 suppressed, 0 new."));
    }

    #[test]
    fn pathless_diagnostic_is_never_suppressed() {
        let mut harness = baseline_harness(
            "baseline-pathless",
            &[entry_for("tool blew up", "x = 1")],
        );
        harness.write_source("src/a.py", "x = 1\n");
        let infrastructure = proto::Diagnostic {
            severity: proto::Severity::Error as i32,
            message: "tool blew up".to_owned(),
            tool_id: "lint-tool".to_owned(),
            rule_id: "lint-tool/rule".to_owned(),
            ..Default::default()
        };
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![infrastructure], vec![]),
        );
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=text"]);
        assert_eq!(code, 1);
        assert!(!out.contains("(suppressed)"));
        assert!(out.contains("Baseline quality-baseline.json: 1 total, 0 suppressed, 1 new."));
    }

    #[test]
    fn json_notice_carries_baseline_counts() {
        let mut harness = baseline_harness("baseline-json", &[entry_for("unused", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, out, _) = run_with_defaults(&harness, &["lint", "--check", "--output=json"]);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let notice = event(&events, "notice");
        assert_eq!(notice["code"], serde_json::json!("baseline"));
        assert_eq!(
            notice["message"],
            serde_json::json!("baseline quality-baseline.json: total=1 suppressed=1 new=0")
        );
        assert_eq!(
            event(&events, "command_finished")["exit_code"],
            serde_json::json!(0)
        );
    }

    #[test]
    fn sarif_marks_suppressed_results() {
        let mut harness = baseline_harness("baseline-sarif", &[entry_for("unused", "x = 1")]);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (code, _, _) = run_with_defaults(
            &harness,
            &["lint", "--check", "--output=text", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 0);
        let document: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif"))
                .expect("valid JSON");
        let results = document["runs"][0]["results"].as_array().expect("results");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0]["suppressions"],
            serde_json::json!([{"kind": "external", "justification": "accepted quality baseline"}])
        );
    }

    #[test]
    fn baseline_paths_stay_inside_the_workspace() {
        let workspace = temp_dir("baseline-paths");
        for rel in ["", "/abs/base.json", "a\\b.json", "../escape.json", "a/../b.json", "dir/"] {
            let error = load_baseline(workspace.path(), rel).expect_err(rel);
            assert!(error.contains("baseline path"), "{rel}: {error}");
        }
        assert!(load_baseline(workspace.path(), "quality-baseline.json").is_ok());
        assert!(load_baseline(workspace.path(), "nested/base.json").is_ok());
    }

    #[test]
    fn missing_baseline_file_reads_as_absent() {
        let workspace = temp_dir("baseline-absent");
        assert!(matches!(
            load_baseline(workspace.path(), "quality-baseline.json"),
            Ok(BaselineInput::Absent)
        ));
    }

    #[test]
    fn unreadable_source_never_matches() {
        let workspace = temp_dir("baseline-unreadable");
        let diagnostic = DiagnosticEvent {
            severity: dx_output::Severity::Warning,
            tool: "lint-tool".to_owned(),
            message: "unused".to_owned(),
            rule: Some("lint-tool/rule".to_owned()),
            path: Some("src/missing.py".to_owned()),
            range: Some((0, 1)),
            snapshot: dx_output::Snapshot::Initial,
            fixable: false,
            resolution: None,
        };
        assert!(fingerprint_diagnostic(workspace.path(), &diagnostic).is_none());
    }
}
