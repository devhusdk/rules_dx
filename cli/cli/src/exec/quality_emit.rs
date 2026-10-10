use std::collections::BTreeMap;
use std::io::Write;

use dx_output::{
    change_value, diagnostic_value, mutation_value, write_event, ChangeKind, DiagnosticEvent,
    MutationOutcome, OutputMode, Resolution, Snapshot,
};

use super::common::{change_event_for, text_diagnostic, FileChange, REASON_INCOMPLETE_COLLECTION};
use crate::args::Invocation;

pub(crate) struct EmitInputs<'a> {
    pub(crate) invocation: &'a Invocation,
    pub(crate) status: &'a [DiagnosticEvent],
    pub(crate) changes: &'a [FileChange],
    pub(crate) applied: &'a BTreeMap<String, bool>,
    pub(crate) not_applied: &'a [(String, &'static str)],
    pub(crate) patch: &'a str,
    pub(crate) stdout_report: bool,
    pub(crate) suppressed: &'a [bool],
    pub(crate) baseline_counts: Option<(usize, usize, usize)>,
    pub(crate) baseline_error: Option<&'a str>,
    pub(crate) baseline_stale: &'a [String],
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
        changes,
        applied,
        not_applied,
        patch,
        stdout_report,
        suppressed,
        baseline_counts,
        baseline_error,
        baseline_stale,
    } = inputs;
    let mut applied_count = 0u64;
    let mut not_applied_count = 0u64;
    if invocation.output == OutputMode::Json {
        let mutating = invocation.applies();
        for (index, diagnostic) in status.iter().enumerate() {
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
            let mut event = diagnostic_value(&event_diagnostic);
            if suppressed.get(index).copied().unwrap_or(false) {
                if let serde_json::Value::Object(ref mut map) = event {
                    map.insert(
                        "suppressed".to_owned(),
                        serde_json::Value::Bool(true),
                    );
                }
            }
            let _ = write_event(out, &event);
        }
        if let Some((total, new, held)) = baseline_counts {
            let _ = write_event(
                out,
                &serde_json::json!({
                    "schema": {"major": 1, "minor": 0},
                    "event": "notice",
                    "level": if new == 0 && baseline_error.is_none() { "info" } else { "warning" },
                    "code": "baseline",
                    "message": format!("baseline: {total} total, {new} new, {held} suppressed"),
                }),
            );
        }
        if let Some(detail) = baseline_error {
            let _ = write_event(
                out,
                &serde_json::json!({
                    "schema": {"major": 1, "minor": 0},
                    "event": "error",
                    "code": "baseline",
                    "message": detail,
                }),
            );
        }
        for stale in baseline_stale {
            let _ = write_event(
                out,
                &serde_json::json!({
                    "schema": {"major": 1, "minor": 0},
                    "event": "notice",
                    "level": "warning",
                    "code": "baseline_stale",
                    "message": format!("baseline stale: {stale}"),
                    "path": stale,
                }),
            );
        }
        for change in changes {
            let _ = write_event(out, &change_value(&change_event_for(change)));
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
            if suppressed.get(index).copied().unwrap_or(false) {
                line.push_str(" (suppressed)");
            }
            let _ = writeln!(human, "{line}");
        }
        if let Some((total, new, held)) = baseline_counts {
            let _ = writeln!(human, "Baseline: {total} total, {new} new, {held} suppressed.");
        }
        if invocation.applies() {
            applied_count = applied.values().filter(|applied| **applied).count() as u64;
            not_applied_count = not_applied.len() as u64;
            if applied_count > 0 {
                let _ = writeln!(human, "Applied {applied_count} file(s).");
            }
        }
        let _ = human.flush();
        if let Some(detail) = baseline_error {
            let _ = writeln!(err, "dx: baseline: {detail}");
        }
        for stale in baseline_stale {
            let _ = writeln!(err, "dx: baseline_stale: {stale}");
        }
        for (path, reason) in not_applied {
            let _ = writeln!(err, "Not applied: {path} ({reason})");
        }
    } else {
        for (path, reason) in not_applied {
            let _ = writeln!(err, "Not applied: {path} ({reason})");
        }
        if let Some((total, new, held)) = baseline_counts {
            let _ = writeln!(err, "Baseline: {total} total, {new} new, {held} suppressed.");
        }
        if let Some(detail) = baseline_error {
            let _ = writeln!(err, "dx: baseline: {detail}");
        }
        for stale in baseline_stale {
            let _ = writeln!(err, "dx: baseline_stale: {stale}");
        }
        out.write_all(patch.as_bytes()).ok();
    }
    EmitCounts {
        applied_count,
        not_applied_count,
    }
}
