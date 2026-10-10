use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dx_apply::{FileSystem, RealFileSystem};
use dx_output::{
    baseline_event, fingerprint, match_baseline, parse_baseline, refresh_entries, render_baseline,
    BaselineCounts, BaselineFile, DiagnosticEvent, Fingerprint,
};

pub(crate) struct BaselineOutcome {
    pub(crate) suppressed: BTreeSet<usize>,
    pub(crate) counts: BaselineCounts,
    pub(crate) stale: Vec<String>,
    pub(crate) refreshed: bool,
    pub(crate) evaluated: bool,
}

fn resolve_selection(workspace: &Path, selection: &str) -> PathBuf {
    let raw = Path::new(selection);
    if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        workspace.join(raw)
    }
}

fn stale_line(entry: &dx_output::BaselineEntry) -> String {
    match &entry.rule {
        Some(rule) => format!("{} {} [{rule}]", entry.tool, entry.path),
        None => format!("{} {}", entry.tool, entry.path),
    }
}

fn read_sources(
    workspace: &Path,
    status: &[DiagnosticEvent],
) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut wanted = BTreeSet::new();
    for diagnostic in status {
        if let Some(path) = &diagnostic.path {
            wanted.insert(path.clone());
        }
    }
    let mut sources = BTreeMap::new();
    for path in wanted {
        sources.insert(
            path.clone(),
            std::fs::read(workspace.join(&path)).ok(),
        );
    }
    sources
}

fn fingerprints(
    status: &[DiagnosticEvent],
    sources: &BTreeMap<String, Option<Vec<u8>>>,
) -> Vec<Option<Fingerprint>> {
    status
        .iter()
        .map(|diagnostic| {
            let bytes = diagnostic
                .path
                .as_ref()
                .and_then(|path| sources.get(path).and_then(|found| found.as_deref()));
            fingerprint(diagnostic, bytes)
        })
        .collect()
}

pub(crate) fn apply_baseline(
    workspace: &Path,
    selection: &str,
    apply: bool,
    complete: bool,
    coverage: &BTreeSet<(String, String)>,
    status: &[DiagnosticEvent],
) -> Result<BaselineOutcome, String> {
    if selection.is_empty() {
        return Err("baseline selection is empty".to_owned());
    }
    let target = resolve_selection(workspace, selection);
    let display = target.display().to_string();
    let sources = read_sources(workspace, status);
    let prints = fingerprints(status, &sources);
    if !complete {
        return Ok(BaselineOutcome {
            suppressed: BTreeSet::new(),
            counts: BaselineCounts {
                total: status.len() as u64,
                suppressed: 0,
                stale: 0,
            },
            stale: Vec::new(),
            refreshed: false,
            evaluated: false,
        });
    }
    let baseline = match std::fs::read(&target) {
        Ok(bytes) => parse_baseline(&bytes)
            .map_err(|error| format!("baseline {display}: {error}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !apply {
                return Err(format!(
                    "baseline {display}: file not found (run with --apply to create it)"
                ));
            }
            BaselineFile::default()
        }
        Err(error) => {
            return Err(format!("baseline {display}: cannot read file: {error}"));
        }
    };
    let mut present: Vec<Fingerprint> = Vec::new();
    let mut present_index: Vec<usize> = Vec::new();
    for (index, print) in prints.iter().enumerate() {
        if let Some(print) = print {
            present.push(print.clone());
            present_index.push(index);
        }
    }
    let matched = match_baseline(&baseline, &present, coverage);
    let suppressed: BTreeSet<usize> = matched
        .suppressed
        .iter()
        .enumerate()
        .filter(|(_, held)| **held)
        .map(|(position, _)| present_index[position])
        .collect();
    let stale: Vec<String> = matched
        .stale
        .iter()
        .map(|index| stale_line(&baseline.entries[*index]))
        .collect();
    let mut refreshed = false;
    if apply {
        let entries = refresh_entries(&baseline, &present, coverage);
        let rendered = render_baseline(&entries);
        if let Some(parent) = target.parent() {
            if !parent.is_dir() {
                return Err(format!(
                    "baseline {display}: parent directory does not exist"
                ));
            }
        }
        RealFileSystem
            .write_atomic(&target, rendered.as_bytes())
            .map_err(|error| format!("baseline {display}: cannot write file: {error}"))?;
        refreshed = true;
        let rewritten =
            parse_baseline(rendered.as_bytes()).map_err(|error| format!("baseline: {error}"))?;
        let confirmed = match_baseline(&rewritten, &present, coverage);
        debug_assert!(confirmed.stale.is_empty());
        let held: BTreeSet<usize> = confirmed
            .suppressed
            .iter()
            .enumerate()
            .filter(|(_, held)| **held)
            .map(|(position, _)| present_index[position])
            .collect();
        return Ok(BaselineOutcome {
            counts: BaselineCounts {
                total: status.len() as u64,
                suppressed: held.len() as u64,
                stale: 0,
            },
            suppressed: held,
            stale: Vec::new(),
            refreshed,
            evaluated: true,
        });
    }
    Ok(BaselineOutcome {
        counts: BaselineCounts {
            total: status.len() as u64,
            suppressed: suppressed.len() as u64,
            stale: stale.len() as u64,
        },
        suppressed,
        stale,
        refreshed,
        evaluated: true,
    })
}

pub(crate) fn baseline_event_for(
    selection: &str,
    outcome: &BaselineOutcome,
) -> Option<serde_json::Value> {
    baseline_event(selection, &outcome.counts, outcome.evaluated).ok()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::test_support::strings;
    use dx_output::{Severity, Snapshot};
    use quality_result::proto::{Capability, Convergence, FileSnapshot, QualityResult, Stage};
    use quality_result::{encode_validated, SCHEMA_MAJOR, SCHEMA_MINOR};

    const SHELL_PATH: &str = "scripts/hello.sh";
    const SHELL_SOURCE: &str = "#!/bin/sh\necho $hello\n";
    const SC2086: &str = "Double quote to prevent globbing and word splitting.";
    const SC2154: &str = "hello is referenced but not assigned.";

    fn shell_event(rule: &str, message: &str) -> DiagnosticEvent {
        DiagnosticEvent {
            severity: Severity::Warning,
            tool: "shellcheck".to_owned(),
            message: message.to_owned(),
            rule: Some(rule.to_owned()),
            path: Some(SHELL_PATH.to_owned()),
            range: Some((15, 21)),
            snapshot: Snapshot::Initial,
            fixable: false,
            resolution: None,
        }
    }

    fn shell_proto(rule: &str, message: &str) -> quality_result::proto::Diagnostic {
        quality_result::proto::Diagnostic {
            severity: quality_result::proto::Severity::Warning as i32,
            message: message.to_owned(),
            tool_id: "shellcheck".to_owned(),
            rule_id: rule.to_owned(),
            path: SHELL_PATH.to_owned(),
            start_byte: Some(15),
            end_byte: Some(21),
            fixable: false,
        }
    }

    fn shell_bytes(
        harness: &Harness,
        rules: &[(&str, &str)],
    ) -> Vec<u8> {
        let initial: Vec<quality_result::proto::Diagnostic> = rules
            .iter()
            .map(|(rule, message)| shell_proto(rule, message))
            .collect();
        let bytes = std::fs::read(harness.workspace.join(SHELL_PATH)).unwrap_or_default();
        let snapshots = vec![FileSnapshot {
            path: SHELL_PATH.to_owned(),
            digest: dx_digest::blake3(&bytes).to_vec(),
        }];
        let result = QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: "//test:corpus".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "shellcheck".to_owned(),
                class_ids: vec!["shell".to_owned()],
                source_paths: vec![SHELL_PATH.to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: snapshots.clone(),
            terminal_snapshot: snapshots,
            initial_diagnostics: initial,
            terminal_diagnostics: vec![],
            replacements: vec![],
        };
        encode_validated(&result).expect("encode")
    }

    fn shell_harness(name: &str, rules: &[(&str, &str)]) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source(SHELL_PATH, SHELL_SOURCE);
        let bytes = shell_bytes(&harness, rules);
        harness.results.insert("//test:corpus".to_owned(), bytes);
        harness
    }

    fn baseline_invocation(words: &[&str], selection: &str) -> crate::args::Invocation {
        let mut invocation = crate::args::parse(&strings(words)).expect("parse");
        invocation.quality_baseline = Some(selection.to_owned());
        invocation
    }

    fn write_baseline(harness: &Harness, selection: &str, rules: &[(&str, &str)]) {
        let source = std::fs::read(harness.workspace.join(SHELL_PATH)).expect("source");
        let entries: Vec<dx_output::BaselineEntry> = rules
            .iter()
            .map(|(rule, message)| {
                let print = dx_output::fingerprint(&shell_event(rule, message), Some(&source))
                    .expect("fingerprint");
                dx_output::BaselineEntry {
                    tool: print.tool,
                    rule: print.rule,
                    path: print.path,
                    message: print.message,
                    context: print.context,
                }
            })
            .collect();
        harness.write_source(selection, &dx_output::render_baseline(&entries));
    }

    #[test]
    fn check_suppresses_baselined_and_fails_new() {
        let harness = shell_harness(
            "baseline-check",
            &[("SC2086", SC2086), ("SC2154", SC2154)],
        );
        write_baseline(&harness, "baselines/lint.json", &[("SC2086", SC2086)]);
        let before = std::fs::read(harness.workspace.join("baselines/lint.json")).expect("read");
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1);
        assert!(out.contains("(suppressed)"), "{out}");
        assert!(out.contains(SC2154), "{out}");
        assert!(
            out.contains("Suppressed 1 of 2 diagnostic(s) via baselines/lint.json."),
            "{out}"
        );
        let after = std::fs::read(harness.workspace.join("baselines/lint.json")).expect("read");
        assert_eq!(before, after, "a check never rewrites the baseline");
    }

    #[test]
    fn check_passes_when_everything_is_baselined() {
        let harness = shell_harness(
            "baseline-clean",
            &[("SC2086", SC2086), ("SC2154", SC2154)],
        );
        write_baseline(
            &harness,
            "baselines/lint.json",
            &[("SC2086", SC2086), ("SC2154", SC2154)],
        );
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Suppressed 2 of 2 diagnostic(s) via baselines/lint.json."),
            "{out}"
        );
    }

    #[test]
    fn json_reports_suppressed_findings_and_counts() {
        let harness = shell_harness("baseline-json", &[("SC2086", SC2086), ("SC2154", SC2154)]);
        write_baseline(&harness, "baselines/lint.json", &[("SC2086", SC2086)]);
        let invocation = baseline_invocation(
            &["lint", "--check", "--output=json"],
            "baselines/lint.json",
        );
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1);
        let events = json_events(&out);
        let baseline = event(&events, "baseline");
        assert_eq!(baseline["path"], serde_json::json!("baselines/lint.json"));
        assert_eq!(baseline["total"], serde_json::json!(2));
        assert_eq!(baseline["suppressed"], serde_json::json!(1));
        assert_eq!(baseline["stale"], serde_json::json!(0));
        assert_eq!(baseline["coverage_complete"], serde_json::json!(true));
        let diagnostics = events_of_kind(&events, "diagnostic");
        assert_eq!(diagnostics.len(), 2);
        let suppressed: Vec<&&serde_json::Value> = diagnostics
            .iter()
            .filter(|event| event.get("suppressed") == Some(&serde_json::json!(true)))
            .collect();
        assert_eq!(suppressed.len(), 1);
        assert_eq!(suppressed[0]["rule"], serde_json::json!("SC2086"));
    }

    #[test]
    fn stale_entries_fail_and_survive_checks() {
        let harness = shell_harness("baseline-stale", &[("SC2154", SC2154)]);
        write_baseline(
            &harness,
            "baselines/lint.json",
            &[("SC2086", SC2086), ("SC2154", SC2154)],
        );
        let before = std::fs::read(harness.workspace.join("baselines/lint.json")).expect("read");
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains("Stale baseline entry: shellcheck scripts/hello.sh [SC2086]"),
            "{out}"
        );
        let after = std::fs::read(harness.workspace.join("baselines/lint.json")).expect("read");
        assert_eq!(before, after, "a check never prunes stale entries");
    }

    #[test]
    fn out_of_scope_entries_stay_silent() {
        let harness = shell_harness("baseline-scope", &[("SC2154", SC2154)]);
        let source = std::fs::read(harness.workspace.join(SHELL_PATH)).expect("source");
        let print =
            dx_output::fingerprint(&shell_event("SC2154", SC2154), Some(&source)).expect("print");
        let mut entries = vec![dx_output::BaselineEntry {
            tool: print.tool,
            rule: print.rule,
            path: "scripts/elsewhere.sh".to_owned(),
            message: print.message,
            context: print.context,
        }];
        entries.push(dx_output::BaselineEntry {
            tool: "shellcheck".to_owned(),
            rule: Some("SC2154".to_owned()),
            path: SHELL_PATH.to_owned(),
            message: SC2154.to_owned(),
            context: print.context,
        });
        harness.write_source("baselines/lint.json", &dx_output::render_baseline(&entries));
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Suppressed 1 of 1 diagnostic(s) via baselines/lint.json."),
            "{out}"
        );
    }

    #[test]
    fn duplicates_need_matching_entry_counts() {
        let harness = shell_harness(
            "baseline-dup",
            &[("SC2086", SC2086), ("SC2086", SC2086)],
        );
        write_baseline(&harness, "baselines/lint.json", &[("SC2086", SC2086)]);
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains("Suppressed 1 of 2 diagnostic(s) via baselines/lint.json."),
            "{out}"
        );
    }

    #[test]
    fn moved_lines_stay_suppressed() {
        let mut harness = Harness::new("baseline-moved");
        harness.write_source(SHELL_PATH, SHELL_SOURCE);
        write_baseline(&harness, "baselines/lint.json", &[("SC2086", SC2086)]);
        harness.write_source(SHELL_PATH, "#!/bin/sh\n# a comment\necho $hello\n");
        let moved = quality_result::proto::Diagnostic {
            severity: quality_result::proto::Severity::Warning as i32,
            message: SC2086.to_owned(),
            tool_id: "shellcheck".to_owned(),
            rule_id: "SC2086".to_owned(),
            path: SHELL_PATH.to_owned(),
            start_byte: Some(27),
            end_byte: Some(33),
            fixable: false,
        };
        let bytes = std::fs::read(harness.workspace.join(SHELL_PATH)).unwrap_or_default();
        let snapshots = vec![FileSnapshot {
            path: SHELL_PATH.to_owned(),
            digest: dx_digest::blake3(&bytes).to_vec(),
        }];
        let result = QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: "//test:corpus".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "shellcheck".to_owned(),
                class_ids: vec!["shell".to_owned()],
                source_paths: vec![SHELL_PATH.to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: snapshots.clone(),
            terminal_snapshot: snapshots,
            initial_diagnostics: vec![moved],
            terminal_diagnostics: vec![],
            replacements: vec![],
        };
        harness
            .results
            .insert("//test:corpus".to_owned(), encode_validated(&result).expect("encode"));
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("(suppressed)"), "{out}");
    }

    #[test]
    fn apply_creates_and_refreshes_the_baseline() {
        let harness = shell_harness("baseline-apply", &[("SC2086", SC2086)]);
        std::fs::create_dir_all(harness.workspace.join("baselines")).expect("mkdir");
        let invocation = baseline_invocation(&["lint", "--apply"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Refreshed baselines/lint.json."), "{out}");
        let written =
            std::fs::read(harness.workspace.join("baselines/lint.json")).expect("created");
        let parsed = dx_output::parse_baseline(&written).expect("created file parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].rule, Some("SC2086".to_owned()));
        let check = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&check, &harness.runner());
        assert_eq!(code, 0, "{out}");
    }

    #[test]
    fn apply_prunes_stale_and_keeps_other_scopes() {
        let harness = shell_harness("baseline-prune", &[("SC2154", SC2154)]);
        write_baseline(
            &harness,
            "baselines/lint.json",
            &[("SC2086", SC2086), ("SC2154", SC2154)],
        );
        let invocation = baseline_invocation(&["lint", "--apply"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 0, "{out}");
        let written =
            std::fs::read(harness.workspace.join("baselines/lint.json")).expect("read");
        let parsed = dx_output::parse_baseline(&written).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].rule, Some("SC2154".to_owned()));
    }

    #[test]
    fn malformed_baseline_fails_without_results() {
        let harness = shell_harness("baseline-malformed", &[("SC2086", SC2086)]);
        harness.write_source("baselines/lint.json", "{\"version\": 1}\n");
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, _, err) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("dx: baseline_failed:"), "{err}");
    }

    #[test]
    fn missing_baseline_fails_checks_and_waits_for_apply() {
        let harness = shell_harness("baseline-missing", &[("SC2086", SC2086)]);
        let invocation = baseline_invocation(&["lint", "--check"], "baselines/lint.json");
        let (code, _, err) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("run with --apply to create it"), "{err}");
        assert!(!harness.workspace.join("baselines/lint.json").exists());
    }

    #[test]
    fn missing_parent_fails_refresh() {
        let harness = shell_harness("baseline-parent", &[("SC2086", SC2086)]);
        let invocation =
            baseline_invocation(&["lint", "--apply"], "nope/baseline.json");
        let (code, _, err) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("parent directory does not exist"), "{err}");
    }

    #[test]
    fn incomplete_results_suppress_nothing_and_refresh_nothing() {
        let mut harness = shell_harness("baseline-incomplete", &[("SC2086", SC2086)]);
        harness.fail_target = true;
        write_baseline(&harness, "baselines/lint.json", &[("SC2086", SC2086)]);
        let invocation = baseline_invocation(
            &["lint", "--check", "--output=json"],
            "baselines/lint.json",
        );
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1);
        let events = json_events(&out);
        let baseline = event(&events, "baseline");
        assert_eq!(baseline["suppressed"], serde_json::json!(0));
        assert_eq!(baseline["stale"], serde_json::json!(0));
        assert_eq!(baseline["coverage_complete"], serde_json::json!(false));
        assert!(
            events_of_kind(&events, "diagnostic")
                .iter()
                .all(|event| event.get("suppressed").is_none()),
            "{out}"
        );
    }

    #[test]
    fn incomplete_apply_writes_nothing() {
        let mut harness =
            shell_harness("baseline-incomplete-apply", &[("SC2086", SC2086)]);
        harness.fail_target = true;
        let invocation =
            baseline_invocation(&["lint", "--apply"], "baselines/lint.json");
        let (code, _, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1);
        assert!(!harness.workspace.join("baselines/lint.json").exists());
    }

    #[test]
    fn apply_fixes_first_and_baselines_what_remains() {
        let mut harness = Harness::new("baseline-fix");
        harness.write_source(SHELL_PATH, SHELL_SOURCE);
        std::fs::create_dir_all(harness.workspace.join("baselines")).expect("mkdir");
        let original = SHELL_SOURCE.as_bytes();
        let fixed = b"#!/bin/sh\necho \"$hello\"\n";
        let initial = vec![quality_result::proto::Diagnostic {
            severity: quality_result::proto::Severity::Warning as i32,
            message: SC2086.to_owned(),
            tool_id: "shellcheck".to_owned(),
            rule_id: "SC2086".to_owned(),
            path: SHELL_PATH.to_owned(),
            start_byte: Some(10),
            end_byte: Some(21),
            fixable: true,
        }];
        let replacements = vec![quality_result::proto::FileEdits {
            path: SHELL_PATH.to_owned(),
            original_digest: dx_digest::blake3(original).to_vec(),
            edits: vec![quality_result::proto::Edit {
                start_byte: 0,
                end_byte: original.len() as u64,
                replacement: fixed.to_vec(),
            }],
        }];
        let snapshots = vec![FileSnapshot {
            path: SHELL_PATH.to_owned(),
            digest: dx_digest::blake3(fixed).to_vec(),
        }];
        let result = QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: "//test:corpus".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "shellcheck".to_owned(),
                class_ids: vec!["shell".to_owned()],
                source_paths: vec![SHELL_PATH.to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: vec![FileSnapshot {
                path: SHELL_PATH.to_owned(),
                digest: dx_digest::blake3(original).to_vec(),
            }],
            terminal_snapshot: snapshots,
            initial_diagnostics: initial,
            terminal_diagnostics: vec![],
            replacements,
        };
        harness
            .results
            .insert("//test:corpus".to_owned(), encode_validated(&result).expect("encode"));
        let invocation = baseline_invocation(&["lint", "--apply"], "baselines/lint.json");
        let (code, out, _) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 0, "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join(SHELL_PATH)).expect("read"),
            fixed,
            "fixes still apply with a baseline configured"
        );
        let written = std::fs::read(harness.workspace.join("baselines/lint.json")).expect("read");
        let parsed = dx_output::parse_baseline(&written).expect("parses");
        assert!(
            parsed.entries.is_empty(),
            "the fixed finding leaves no baseline behind"
        );
    }

    #[test]
    fn sarif_marks_baselined_results() {
        let harness = shell_harness("baseline-sarif", &[("SC2086", SC2086), ("SC2154", SC2154)]);
        write_baseline(&harness, "baselines/lint.json", &[("SC2086", SC2086)]);
        let invocation = baseline_invocation(
            &[
                "lint",
                "--check",
                "--output=text",
                "--report=sarif=out.sarif",
            ],
            "baselines/lint.json",
        );
        let (code, _, err) = harness.execute_with(&invocation, &harness.runner());
        assert_eq!(code, 1, "{err}");
        let document = std::fs::read(harness.workspace.join("out.sarif")).expect("sarif");
        let sarif: serde_json::Value = serde_json::from_slice(&document).expect("sarif json");
        let results = sarif["runs"][0]["results"].as_array().expect("results");
        assert_eq!(results.len(), 2);
        let mut states: Vec<&str> = results
            .iter()
            .map(|result| result["baselineState"].as_str().expect("baselineState"))
            .collect();
        states.sort();
        assert_eq!(states, vec!["new", "unchanged"]);
    }
}
