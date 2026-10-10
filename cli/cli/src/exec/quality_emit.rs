use std::collections::BTreeMap;
use std::io::Write;

use dx_output::{
    change_value, diagnostic_value, mutation_value, notice_event, write_event, ChangeKind,
    DiagnosticEvent, MutationOutcome, NoticeEvent, OutputMode, Resolution, Snapshot,
};

use super::common::{change_event_for, text_diagnostic, FileChange, REASON_INCOMPLETE_COLLECTION};
use super::quality_baseline::{
    notice_message, refreshed_message, stale_line, summary_message, BaselineReport,
    SUPPRESSED_MARK,
};
use crate::args::Invocation;

pub(crate) struct EmitInputs<'a> {
    pub(crate) invocation: &'a Invocation,
    pub(crate) status: &'a [DiagnosticEvent],
    pub(crate) suppressed: &'a [bool],
    pub(crate) baseline: Option<&'a BaselineReport>,
    pub(crate) changes: &'a [FileChange],
    pub(crate) applied: &'a BTreeMap<String, bool>,
    pub(crate) not_applied: &'a [(String, &'static str)],
    pub(crate) patch: &'a str,
    pub(crate) stdout_report: bool,
}

pub(crate) struct EmitCounts {
    pub(crate) applied_count: u64,
    pub(crate) not_applied_count: u64,
}

pub(crate) fn emit_findings(
    inputs: EmitInputs<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> EmitCounts {
    let EmitInputs {
        invocation,
        status,
        suppressed,
        baseline,
        changes,
        applied,
        not_applied,
        patch,
        stdout_report,
    } = inputs;
    let suppressed_flag = |index: usize| suppressed.get(index).copied().unwrap_or(false);
    let mut applied_count = 0u64;
    let mut not_applied_count = 0u64;
    if invocation.output == OutputMode::Json {
        let mutating = invocation.applies();
        for diagnostic in status {
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
            let _ = write_event(out, &diagnostic_value(&event_diagnostic));
        }
        for change in changes {
            let _ = write_event(out, &change_value(&change_event_for(change)));
        }
        if let Some(report) = baseline {
            let notice = NoticeEvent {
                level: "info".to_owned(),
                code: "baseline".to_owned(),
                message: notice_message(report),
                related_command: None,
                scope: None,
                path: None,
                language: None,
                import: None,
            };
            if let Ok(event) = notice_event(&notice) {
                let _ = write_event(out, &event);
            }
        }
        if mutating {
            for change in changes {
                let is_applied = applied.get(&change.path).copied().unwrap_or(false);
                let reason = if is_applied {
                    None
                } else {
                    Some(
                        not_applied
                            .iter()
                            .find(|(path, _)| path == &change.path)
                            .map(|(_, reason)| *reason)
                            .unwrap_or(REASON_INCOMPLETE_COLLECTION),
                    )
                };
                if is_applied {
                    applied_count += 1;
                } else {
                    not_applied_count += 1;
                }
                let _ = write_event(
                    out,
                    &mutation_value(
                        &change.path,
                        ChangeKind::Modify,
                        if is_applied {
                            MutationOutcome::Applied
                        } else {
                            MutationOutcome::NotApplied
                        },
                        reason,
                    ),
                );
            }
        }
    } else if matches!(invocation.output, OutputMode::Text { .. }) {
        let human: &mut dyn Write = if stdout_report { err } else { out };
        for (index, diagnostic) in status.iter().enumerate() {
            let mut line = text_diagnostic(diagnostic);
            if suppressed_flag(index) {
                line.push_str(SUPPRESSED_MARK);
            }
            let _ = writeln!(human, "{line}");
        }
        if let Some(report) = baseline {
            let _ = writeln!(human, "{}", summary_message(report));
            if let Some(refreshed) = refreshed_message(report) {
                let _ = writeln!(human, "{refreshed}");
            }
            for entry in &report.stale {
                let _ = writeln!(human, "{}", stale_line(entry));
            }
        }
        if invocation.applies() {
            applied_count = applied.values().filter(|applied| **applied).count() as u64;
            not_applied_count = not_applied.len() as u64;
            if applied_count > 0 {
                let _ = writeln!(human, "Applied {applied_count} file(s).");
            }
        }
        for (path, reason) in not_applied {
            let _ = writeln!(err, "Not applied: {path} ({reason})");
        }
    } else {
        for (path, reason) in not_applied {
            let _ = writeln!(err, "Not applied: {path} ({reason})");
        }
        out.write_all(patch.as_bytes()).ok();
    }
    EmitCounts {
        applied_count,
        not_applied_count,
    }
}
