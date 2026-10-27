use std::collections::BTreeMap;
use std::io::Write;

use dx_output::{
    change_value, finding_value, refusal_value, ChangeEffect, ChangeKind, DiagnosticEvent,
    FindingRecord, OutputMode, Position, Resolution, Snapshot, Span,
};

use super::common::{
    change_event_for, check_stdout_write, emit_event, text_finding, FileChange,
    REASON_INCOMPLETE_COLLECTION,
};
use super::quality_baseline::BaselineView;
use super::quality_reconcile::Conflict;
use super::quality_reports::Snapshots;
use dx_plan::args::QualityRequest;
use dx_plan::reports::sarif::byte_to_line;

pub(crate) struct EmitInputs<'a> {
    pub(crate) request: &'a QualityRequest,
    pub(crate) status: &'a [DiagnosticEvent],
    pub(crate) changes: &'a [FileChange],
    pub(crate) applied: &'a BTreeMap<String, bool>,
    pub(crate) not_applied: &'a [(String, &'static str)],
    pub(crate) conflicts: &'a [Conflict],
    pub(crate) patch: &'a str,
    pub(crate) stdout_report: bool,
    pub(crate) suppressed: &'a [bool],
    pub(crate) baseline: Option<&'a BaselineView>,
    pub(crate) snapshots: &'a Snapshots,
    pub(crate) original_digests: &'a BTreeMap<String, [u8; 32]>,
    pub(crate) terminal_digests: &'a BTreeMap<String, [u8; 32]>,
}

pub(crate) struct EmitCounts {
    pub(crate) applied_count: u64,
    pub(crate) not_applied_count: u64,
}

fn span_of(
    snapshots: &Snapshots,
    snapshot: Snapshot,
    path: Option<&String>,
    range: Option<(u64, u64)>,
) -> Option<Span> {
    let path = path?;
    let (start, end) = range?;
    let text = snapshots.get(&(snapshot, path.clone()))?;
    let (start_line, start_column) = byte_to_line(path, text, start).ok()?;
    let (end_line, end_column) = byte_to_line(path, text, end).ok()?;
    Some((
        Position {
            line: start_line,
            column: start_column,
        },
        Position {
            line: end_line,
            column: end_column,
        },
    ))
}

fn same_place(a: &DiagnosticEvent, b: &DiagnosticEvent) -> bool {
    a.snapshot == b.snapshot && a.path == b.path && a.range == b.range
}

fn equivalent(a: &DiagnosticEvent, b: &DiagnosticEvent) -> bool {
    same_place(a, b)
        && a.severity == b.severity
        && a.tool == b.tool
        && a.rule == b.rule
        && a.message == b.message
        && a.fixable == b.fixable
        && a.resolution == b.resolution
        && a.related == b.related
        && a.package == b.package
}

/// Folds findings proven equal on one verified snapshot and attaches their verified lines.
pub(crate) fn finding_records(
    status: &[DiagnosticEvent],
    suppressed: &[bool],
    snapshots: &Snapshots,
    original_digests: &BTreeMap<String, [u8; 32]>,
    terminal_digests: &BTreeMap<String, [u8; 32]>,
) -> Vec<FindingRecord> {
    let mut records: Vec<FindingRecord> = Vec::new();
    for (index, diagnostic) in status.iter().enumerate() {
        let held = suppressed.get(index).copied().unwrap_or(false);
        let proven = diagnostic
            .path
            .as_ref()
            .is_some_and(|path| match diagnostic.snapshot {
                Snapshot::Initial => original_digests.contains_key(path),
                Snapshot::Terminal => terminal_digests.contains_key(path),
            });
        if proven {
            let folded = records
                .iter_mut()
                .rev()
                .take_while(|record| same_place(&record.diagnostic, diagnostic))
                .find(|record| {
                    record.suppressed == held && equivalent(&record.diagnostic, diagnostic)
                });
            if let Some(record) = folded {
                record.count += 1;
                for origin in &diagnostic.origins {
                    if !record.diagnostic.origins.contains(origin) {
                        record.diagnostic.origins.push(origin.clone());
                    }
                }
                record.diagnostic.origins.sort();
                continue;
            }
        }
        let mut record = FindingRecord::single(diagnostic.clone());
        record.suppressed = held;
        record.span = span_of(
            snapshots,
            diagnostic.snapshot,
            diagnostic.path.as_ref(),
            diagnostic.range,
        );
        record.related_spans = diagnostic
            .related
            .iter()
            .map(|item| {
                span_of(
                    snapshots,
                    diagnostic.snapshot,
                    item.path.as_ref(),
                    item.range,
                )
            })
            .collect();
        records.push(record);
    }
    records
}

pub(crate) fn emit_findings(
    inputs: EmitInputs<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<EmitCounts, i32> {
    let EmitInputs {
        request,
        status,
        changes,
        applied,
        not_applied,
        conflicts,
        patch,
        stdout_report,
        suppressed,
        baseline,
        snapshots,
        original_digests,
        terminal_digests,
    } = inputs;
    let mut applied_count = 0u64;
    let mut not_applied_count = 0u64;
    if request.common.output == OutputMode::Json {
        let mutating = request.applies();
        let resolved: Vec<DiagnosticEvent> = status
            .iter()
            .map(|diagnostic| {
                let mut event_diagnostic = diagnostic.clone();
                if mutating && event_diagnostic.snapshot == Snapshot::Initial {
                    let is_applied = event_diagnostic
                        .path
                        .as_ref()
                        .is_some_and(|path| applied.get(path).copied().unwrap_or(false));
                    event_diagnostic.resolution = Some(if is_applied {
                        Resolution::Remaining
                    } else {
                        Resolution::NotApplied
                    });
                }
                event_diagnostic
            })
            .collect();
        for record in finding_records(
            &resolved,
            suppressed,
            snapshots,
            original_digests,
            terminal_digests,
        ) {
            emit_event(out, &finding_value(&record))?;
        }
        let include_edits = request.common.include_edits;
        for change in changes {
            let (effect, reason) = if !mutating {
                (ChangeEffect::Proposed, None)
            } else if applied.get(&change.path).copied().unwrap_or(false) {
                applied_count += 1;
                (ChangeEffect::Applied, None)
            } else {
                not_applied_count += 1;
                (
                    ChangeEffect::Refused,
                    Some(
                        not_applied
                            .iter()
                            .find(|(path, _)| path == &change.path)
                            .map(|(_, reason)| *reason)
                            .unwrap_or(REASON_INCOMPLETE_COLLECTION),
                    ),
                )
            };
            emit_event(
                out,
                &change_value(&change_event_for(change), effect, reason, include_edits),
            )?;
        }
        for conflict in conflicts {
            if mutating {
                not_applied_count += 1;
            }
            emit_event(
                out,
                &refusal_value(&conflict.path, ChangeKind::Modify, conflict.reason),
            )?;
        }
    } else if matches!(request.common.output, OutputMode::Text { .. }) {
        let human: &mut dyn Write = if stdout_report { err } else { out };
        let required = !stdout_report;
        for record in finding_records(
            status,
            suppressed,
            snapshots,
            original_digests,
            terminal_digests,
        ) {
            let mut line = text_finding(&record);
            if record.suppressed {
                line.push_str(" (baselined)");
            }
            human_line(human, required, &line)?;
        }
        if request.applies() {
            applied_count = applied.values().filter(|applied| **applied).count() as u64;
            not_applied_count = not_applied.len() as u64;
            if applied_count > 0 {
                human_line(
                    human,
                    required,
                    &format!("Applied {applied_count} file(s)."),
                )?;
            }
        }
        for (path, reason) in not_applied {
            let _ = writeln!(err, "Not applied: {path} ({reason})");
        }
    } else {
        for (path, reason) in not_applied {
            let _ = writeln!(err, "Not applied: {path} ({reason})");
        }
        check_stdout_write(out.write_all(patch.as_bytes()))?;
    }
    for conflict in conflicts {
        let _ = writeln!(
            err,
            "Conflict: {} ({}; producers: {})",
            conflict.path,
            conflict.reason,
            conflict.producers.join(", ")
        );
    }
    if let Some(view) = baseline {
        for stale in &view.stale {
            let _ = writeln!(
                err,
                "Baseline stale: {stale} (run with --apply to refresh)."
            );
        }
        if matches!(request.common.output, OutputMode::Text { .. }) {
            let human: &mut dyn Write = if stdout_report { err } else { out };
            let required = !stdout_report;
            human_line(
                human,
                required,
                &format!(
                    "Baseline {}: {} total, {} new, {} suppressed.",
                    view.rel, view.total, view.fresh, view.held
                ),
            )?;
            if view.refreshed {
                human_line(
                    human,
                    required,
                    &format!("Baseline {} refreshed.", view.rel),
                )?;
            }
        }
    }
    Ok(EmitCounts {
        applied_count,
        not_applied_count,
    })
}

fn human_line(human: &mut dyn Write, required: bool, line: &str) -> Result<(), i32> {
    let result = writeln!(human, "{line}");
    if required {
        check_stdout_write(result)
    } else {
        Ok(())
    }
}
