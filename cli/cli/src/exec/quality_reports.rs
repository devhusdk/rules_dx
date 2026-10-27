use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use dx_digest::blake3 as digest;
use dx_output::{report_event, DiagnosticEvent, OutputMode, Snapshot};

use super::common::{
    apply_to_bytes, check_stdout_write, emit_built, report_failed, write_report_document,
    SourceRead,
};
use super::results::Collected;
use dx_cli_core::reports::{
    render_sarif_with_baseline, BaselineReport, PlannedReport, ReportError,
};

pub(crate) struct StandardReports<'a> {
    pub(crate) workspace: &'a Path,
    pub(crate) collected: &'a Collected,
    pub(crate) sources: &'a BTreeMap<String, SourceRead>,
    pub(crate) status: &'a [DiagnosticEvent],
    pub(crate) planned: &'a [PlannedReport],
    pub(crate) output: &'a OutputMode,
    pub(crate) stdout_report: bool,
    pub(crate) baseline: Option<BaselineReport>,
}

pub(crate) type Snapshots = BTreeMap<(Snapshot, String), String>;

fn missing(path: &str) -> ReportError {
    ReportError::MissingSnapshot {
        path: path.to_owned(),
    }
}

fn snapshot_bytes(
    workspace: &Path,
    collected: &Collected,
    sources: &BTreeMap<String, SourceRead>,
    snapshot: Snapshot,
    path: &str,
) -> Result<Vec<u8>, ReportError> {
    let digests = match snapshot {
        Snapshot::Initial => &collected.original_digests,
        Snapshot::Terminal => &collected.terminal_digests,
    };
    let expected = digests.get(path).ok_or_else(|| missing(path))?;
    let mut candidates: Vec<Vec<u8>> = Vec::new();
    if let Ok(bytes) = std::fs::read(workspace.join(path)) {
        candidates.push(bytes);
    }
    if let Some(SourceRead::Bytes(original)) = sources.get(path) {
        candidates.push(original.clone());
        if snapshot == Snapshot::Terminal {
            if let Some(change) = collected.changes.iter().find(|change| change.path == path) {
                if let Some(bytes) = apply_to_bytes(original, &change.edits) {
                    candidates.push(bytes);
                }
            }
        }
    }
    candidates
        .into_iter()
        .find(|bytes| digest(bytes) == *expected)
        .ok_or_else(|| missing(path))
}

fn load_snapshots(
    workspace: &Path,
    collected: &Collected,
    sources: &BTreeMap<String, SourceRead>,
    status: &[DiagnosticEvent],
) -> (Snapshots, Result<(), ReportError>) {
    let mut needed: BTreeSet<(Snapshot, &str)> = BTreeSet::new();
    for finding in status {
        if let (Some(path), Some(_)) = (&finding.path, finding.range) {
            needed.insert((finding.snapshot, path.as_str()));
        }
    }
    let mut snapshots = Snapshots::new();
    for (snapshot, path) in needed {
        let text = snapshot_bytes(workspace, collected, sources, snapshot, path)
            .and_then(|bytes| String::from_utf8(bytes).map_err(|_| missing(path)));
        match text {
            Ok(text) => {
                snapshots.insert((snapshot, path.to_owned()), text);
            }
            Err(error) => return (snapshots, Err(error)),
        }
    }
    let optional: BTreeSet<(Snapshot, &str)> = status
        .iter()
        .flat_map(|finding| {
            finding.related.iter().filter_map(|item| {
                let path = item.path.as_deref()?;
                item.range?;
                Some((finding.snapshot, path))
            })
        })
        .collect();
    for (snapshot, path) in optional {
        if snapshots.contains_key(&(snapshot, path.to_owned())) {
            continue;
        }
        let text = snapshot_bytes(workspace, collected, sources, snapshot, path)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        if let Some(text) = text {
            snapshots.insert((snapshot, path.to_owned()), text);
        }
    }
    (snapshots, Ok(()))
}

/// Loads every source snapshot a finding or related location points into, when its digest verifies.
pub(crate) fn verified_snapshots(
    workspace: &Path,
    collected: &Collected,
    sources: &BTreeMap<String, SourceRead>,
    status: &[DiagnosticEvent],
) -> Snapshots {
    let mut needed: BTreeSet<(Snapshot, &str)> = BTreeSet::new();
    for finding in status {
        if let (Some(path), Some(_)) = (&finding.path, finding.range) {
            needed.insert((finding.snapshot, path.as_str()));
        }
        for item in &finding.related {
            if let (Some(path), Some(_)) = (&item.path, item.range) {
                needed.insert((finding.snapshot, path.as_str()));
            }
        }
    }
    needed
        .into_iter()
        .filter_map(|(snapshot, path)| {
            let bytes = snapshot_bytes(workspace, collected, sources, snapshot, path).ok()?;
            Some(((snapshot, path.to_owned()), String::from_utf8(bytes).ok()?))
        })
        .collect()
}

pub(crate) fn write_standard_reports(
    inputs: StandardReports<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let StandardReports {
        workspace,
        collected,
        sources,
        status,
        planned,
        output,
        stdout_report,
        baseline,
    } = inputs;
    let mut reports_ok = true;
    let mut failures: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &collected.unstable {
        for tool in &item.tools {
            failures
                .entry(tool.clone())
                .or_default()
                .push(item.describe());
        }
    }
    for planned in planned {
        let (snapshots, snapshot_result) = load_snapshots(workspace, collected, sources, status);
        let document = match snapshot_result {
            Err(error) => Err(error),
            Ok(()) => render_sarif_with_baseline(
                &collected.tools,
                status,
                &snapshots,
                collected.complete,
                &failures,
                baseline.as_ref(),
            ),
        };
        match document {
            Ok(document) => {
                if let Err(error) =
                    write_report_document(out, workspace, &planned.destination, &document)
                {
                    reports_ok = false;
                    report_failed(
                        out,
                        err,
                        *output,
                        &format!(
                            "failed to write {} report to {error}",
                            planned.format.name(),
                        ),
                    )?;
                    continue;
                }
                if *output == OutputMode::Json {
                    emit_built(
                        out,
                        report_event(
                            planned.format.name(),
                            planned.destination.display(),
                            collected.complete,
                        ),
                    )?;
                } else if matches!(output, OutputMode::Text { .. }) && !stdout_report {
                    check_stdout_write(writeln!(
                        out,
                        "Wrote {} report to {}.",
                        planned.format.name(),
                        planned.destination.display()
                    ))?;
                } else if *output == OutputMode::Diff {
                    let _ = writeln!(
                        err,
                        "Wrote {} report to {}.",
                        planned.format.name(),
                        planned.destination.display()
                    );
                }
            }
            Err(error) => {
                reports_ok = false;
                report_failed(
                    out,
                    err,
                    *output,
                    &format!("failed to render SARIF report: {error}"),
                )?;
            }
        }
    }
    Ok(reports_ok)
}

#[cfg(test)]
mod tests {
    use super::super::common::FileChange;
    use super::super::test_support::temp_dir;
    use super::*;
    use dx_output::Severity;

    const ORIGINAL: &[u8] = "é = 1\r\nbad\r\n".as_bytes();
    const TERMINAL: &str = "é = 1\r\n";

    fn finding(snapshot: Snapshot, start: u64) -> DiagnosticEvent {
        DiagnosticEvent {
            path: Some("a.py".to_owned()),
            range: Some((start, start + 1)),
            snapshot,
            ..DiagnosticEvent::new(Severity::Warning, "lint", "m")
        }
    }

    fn collected() -> Collected {
        Collected {
            tools: vec!["lint".to_owned()],
            ran: std::collections::BTreeSet::new(),
            analyzed: std::collections::BTreeSet::new(),
            initial: Vec::new(),
            terminal: Vec::new(),
            changes: vec![FileChange {
                path: "a.py".to_owned(),
                original_digest: digest(ORIGINAL),
                edits: vec![(9, ORIGINAL.len() as u64, String::new())],
            }],
            terminal_digests: BTreeMap::from([("a.py".to_owned(), digest(TERMINAL.as_bytes()))]),
            original_digests: BTreeMap::from([("a.py".to_owned(), digest(ORIGINAL))]),
            conflicts: Vec::new(),
            unstable: Vec::new(),
            complete: true,
            problems: Vec::new(),
        }
    }

    fn sources() -> BTreeMap<String, SourceRead> {
        BTreeMap::from([("a.py".to_owned(), SourceRead::Bytes(ORIGINAL.to_vec()))])
    }

    #[test]
    fn check_reads_the_unchanged_original_even_when_the_candidate_differs() {
        let workspace = temp_dir("reports-check");
        std::fs::write(workspace.path().join("a.py"), ORIGINAL).expect("write");
        let status = [finding(Snapshot::Initial, 9)];
        let (snapshots, result) =
            load_snapshots(workspace.path(), &collected(), &sources(), &status);
        assert!(result.is_ok());
        assert_eq!(
            snapshots
                .get(&(Snapshot::Initial, "a.py".to_owned()))
                .map(String::as_bytes),
            Some(ORIGINAL)
        );
    }

    #[test]
    fn applied_run_keeps_original_and_terminal_versions_apart() {
        let workspace = temp_dir("reports-applied");
        std::fs::write(workspace.path().join("a.py"), TERMINAL).expect("write");
        let status = [
            finding(Snapshot::Initial, 9),
            finding(Snapshot::Terminal, 0),
        ];
        let (snapshots, result) =
            load_snapshots(workspace.path(), &collected(), &sources(), &status);
        assert!(result.is_ok());
        assert_eq!(snapshots.len(), 2);
        assert_eq!(
            snapshots[&(Snapshot::Initial, "a.py".to_owned())].as_bytes(),
            ORIGINAL
        );
        assert_eq!(
            snapshots[&(Snapshot::Terminal, "a.py".to_owned())],
            TERMINAL
        );
    }

    #[test]
    fn intervening_edit_is_refused() {
        let workspace = temp_dir("reports-edited");
        std::fs::write(workspace.path().join("a.py"), "other\n").expect("write");
        let status = [finding(Snapshot::Terminal, 0)];
        let (_, result) = load_snapshots(workspace.path(), &collected(), &BTreeMap::new(), &status);
        assert!(matches!(result, Err(ReportError::MissingSnapshot { .. })));
    }

    #[test]
    fn missing_digest_is_refused() {
        let workspace = temp_dir("reports-nodigest");
        std::fs::write(workspace.path().join("a.py"), ORIGINAL).expect("write");
        let mut collected = collected();
        collected.original_digests.clear();
        let status = [finding(Snapshot::Initial, 9)];
        let (_, result) = load_snapshots(workspace.path(), &collected, &sources(), &status);
        assert!(matches!(result, Err(ReportError::MissingSnapshot { .. })));
    }
}
