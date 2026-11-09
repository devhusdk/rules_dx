use std::collections::BTreeMap;
use std::io::Write;

use dx_output::{
    ChangeEffect, ChangeKind, DiagnosticEvent, FindingRecord, NoticeEvent, OutputMode, Position,
    Resolution, Snapshot, Span, change_value, finding_value, notice_event, refusal_value,
};

use super::common::{
    FileChange, REASON_INCOMPLETE_COLLECTION, change_event_for, check_stdout_write, emit_built,
    emit_event, text_finding,
};
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
    pub(crate) snapshots: &'a Snapshots,
    pub(crate) original_digests: &'a BTreeMap<String, [u8; 32]>,
    pub(crate) terminal_digests: &'a BTreeMap<String, [u8; 32]>,
}

pub(crate) struct EmitCounts {
    pub(crate) applied_count: u64,
    pub(crate) not_applied_count: u64,
    pub(crate) findings_omitted: u64,
}

/// Findings per tool a JSON run prints when `--max-findings` is not given.
const DEFAULT_MAX_FINDINGS: u32 = 50;

/// Keeps the first `limit` records of each tool and says how many it dropped; 0 keeps all.
fn cap_per_tool(records: Vec<FindingRecord>, limit: u32) -> (Vec<FindingRecord>, BTreeMap<String, u64>) {
    let mut seen: BTreeMap<String, u64> = BTreeMap::new();
    let mut omitted: BTreeMap<String, u64> = BTreeMap::new();
    let mut kept = Vec::new();
    for record in records {
        let count = seen.entry(record.diagnostic.tool.clone()).or_insert(0);
        *count += 1;
        if limit == 0 || *count <= u64::from(limit) {
            kept.push(record);
        } else {
            *omitted.entry(record.diagnostic.tool.clone()).or_insert(0) += 1;
        }
    }
    (kept, omitted)
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
    snapshots: &Snapshots,
    original_digests: &BTreeMap<String, [u8; 32]>,
    terminal_digests: &BTreeMap<String, [u8; 32]>,
) -> Vec<FindingRecord> {
    let mut records: Vec<FindingRecord> = Vec::new();
    for diagnostic in status {
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
                .find(|record| equivalent(&record.diagnostic, diagnostic));
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
        snapshots,
        original_digests,
        terminal_digests,
    } = inputs;
    let mut applied_count = 0u64;
    let mut not_applied_count = 0u64;
    let mut findings_omitted = 0u64;
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
        let records = finding_records(&resolved, snapshots, original_digests, terminal_digests);
        let limit = request.common.max_findings.unwrap_or(DEFAULT_MAX_FINDINGS);
        let (records, omitted) = cap_per_tool(records, limit);
        for record in &records {
            emit_event(out, &finding_value(record))?;
        }
        findings_omitted = omitted.values().sum();
        if findings_omitted > 0 {
            let tools: Vec<String> = omitted
                .iter()
                .map(|(tool, count)| format!("{tool}: {count}"))
                .collect();
            let notice = NoticeEvent {
                related_command: Some(request.command.name().to_owned()),
                ..NoticeEvent::info(
                    "findings_truncated",
                    format!(
                        "{findings_omitted} finding(s) left out ({}); the command_finished counts are exact. Raise --max-findings or write a SARIF report with --report sarif=<dest> to see them.",
                        tools.join(", ")
                    ),
                )
            };
            emit_built(out, notice_event(&notice))?;
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
        for record in finding_records(status, snapshots, original_digests, terminal_digests) {
            let line = text_finding(&record);
            human_line(human, required, &line)?;
        }
        if request.applies() {
            for (path, _) in applied.iter().filter(|(_, written)| **written) {
                human_line(human, required, &format!("Applied: {path}"))?;
            }
            applied_count = applied.values().filter(|applied| **applied).count() as u64;
            not_applied_count = not_applied.len() as u64;
            if applied_count > 0 {
                human_line(
                    human,
                    required,
                    &format!("Applied {applied_count} file(s)."),
                )?;
            }
            if request.unsafe_fixes && request.chatty() {
                human_line(
                    human,
                    required,
                    "--unsafe: fixes the tools mark unsafe were included; review them.",
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
    if !conflicts.is_empty() {
        let _ = writeln!(
            err,
            "next: resolve the Conflict files listed above by hand or fix one producer, then run the command again."
        );
    }
    Ok(EmitCounts {
        applied_count,
        not_applied_count,
        findings_omitted,
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

#[cfg(test)]
mod tests {
    use super::*;
    use dx_output::Severity;

    fn record(tool: &str, n: u64) -> FindingRecord {
        FindingRecord {
            diagnostic: DiagnosticEvent::new(Severity::Warning, tool, format!("finding {n}")),
            span: None,
            related_spans: Vec::new(),
            count: 1,
        }
    }

    #[test]
    fn the_cap_applies_to_each_tool_and_counts_what_it_drops() {
        let mut records: Vec<FindingRecord> = (0..120).map(|n| record("ruff", n)).collect();
        records.extend((0..3).map(|n| record("clippy", n)));
        let (kept, omitted) = cap_per_tool(records, 50);
        assert_eq!(kept.len(), 53);
        assert_eq!(omitted.get("ruff"), Some(&70));
        assert!(omitted.get("clippy").is_none());
    }

    #[test]
    fn zero_keeps_every_finding() {
        let records: Vec<FindingRecord> = (0..120).map(|n| record("ruff", n)).collect();
        let (kept, omitted) = cap_per_tool(records, 0);
        assert_eq!(kept.len(), 120);
        assert!(omitted.is_empty());
    }
}
