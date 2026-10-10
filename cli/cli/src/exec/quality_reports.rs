use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use dx_digest::blake3 as digest;
use dx_output::{report_event, write_event, DiagnosticEvent, OutputMode};

use super::common::{report_failed, write_report_document};
use super::results::Collected;
use crate::reports::{render_sarif_with_baseline, PlannedReport, ReportError};

pub(crate) struct StandardReports<'a> {
    pub(crate) workspace: &'a Path,
    pub(crate) collected: &'a Collected,
    pub(crate) status: &'a [DiagnosticEvent],
    pub(crate) planned: &'a [PlannedReport],
    pub(crate) output: &'a OutputMode,
    pub(crate) stdout_report: bool,
    pub(crate) suppressed: &'a [bool],
    pub(crate) baseline_counts: Option<(usize, usize, usize)>,
}

pub(crate) fn write_standard_reports(
    inputs: StandardReports<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> bool {
    let StandardReports {
        workspace,
        collected,
        status,
        planned,
        output,
        stdout_report,
        suppressed,
        baseline_counts,
    } = inputs;
    let mut reports_ok = true;
    for planned in planned {
        let mut snapshots = BTreeMap::new();
        let mut snapshot_result: Result<(), ReportError> = Ok(());
        let mut needed: BTreeSet<&str> = BTreeSet::new();
        for finding in status {
            if let Some(path) = &finding.path {
                if finding.range.is_some() {
                    needed.insert(path.as_str());
                }
            }
        }
        for path in needed {
            match std::fs::read(workspace.join(path)) {
                Err(_) => {
                    snapshot_result = Err(ReportError::MissingSnapshot {
                        path: path.to_owned(),
                    });
                    break;
                }
                Ok(bytes) => {
                    if let Some(expected) = collected.terminal_digests.get(path) {
                        if digest(&bytes) != *expected {
                            snapshot_result = Err(ReportError::MissingSnapshot {
                                path: path.to_owned(),
                            });
                            break;
                        }
                    }
                    match String::from_utf8(bytes) {
                        Ok(text) => {
                            snapshots.insert(path.to_owned(), text);
                        }
                        Err(_) => {
                            snapshot_result = Err(ReportError::MissingSnapshot {
                                path: path.to_owned(),
                            });
                            break;
                        }
                    }
                }
            }
        }
        let document = match snapshot_result {
            Err(error) => Err(error),
            Ok(()) => render_sarif_with_baseline(
                &collected.tools,
                status,
                &snapshots,
                collected.complete,
                suppressed,
                baseline_counts,
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
                    );
                    continue;
                }
                if *output == OutputMode::Json {
                    if let Ok(event) = report_event(
                        planned.format.name(),
                        planned.destination.display(),
                        collected.complete,
                    ) {
                        let _ = write_event(out, &event);
                    }
                } else if matches!(output, OutputMode::Text { .. }) && !stdout_report {
                    let _ = writeln!(
                        out,
                        "Wrote {} report to {}.",
                        planned.format.name(),
                        planned.destination.display()
                    );
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
                );
            }
        }
    }
    reports_ok
}
