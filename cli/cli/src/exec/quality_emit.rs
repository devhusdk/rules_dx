use std::collections::BTreeMap;
use std::io::Write;

use dx_output::{
    change_event, diagnostic_event, mutation_event, write_event, ChangeKind, DiagnosticEvent,
    MutationOutcome, OutputMode, Resolution, Snapshot,
};

use super::common::{
    change_event_for, text_diagnostic, FileChange, CODE_INVALID_BEP, REASON_INCOMPLETE_COLLECTION,
};
use crate::args::Invocation;

pub(crate) struct EmitInputs<'a> {
    pub(crate) invocation: &'a Invocation,
    pub(crate) status: &'a [DiagnosticEvent],
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
) -> Result<EmitCounts, (&'static str, String)> {
    let EmitInputs {
        invocation,
        status,
        changes,
        applied,
        not_applied,
        patch,
        stdout_report,
    } = inputs;
    let mut applied_count = 0u64;
    let mut not_applied_count = 0u64;
    if invocation.output == OutputMode::Json {
        let mutating = !invocation.check;
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
            match diagnostic_event(&event_diagnostic, mutating) {
                Ok(event) => {
                    let _ = write_event(out, &event);
                }
                // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                Err(error) => {
                    return Err((
                        CODE_INVALID_BEP,
                        format!("invalid finding for output: {error}"),
                    ));
                } // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
            }
        }
        for change in changes {
            match change_event_for(change) {
                Ok(change_event_value) => match change_event(&change_event_value) {
                    Ok(event) => {
                        let _ = write_event(out, &event);
                    }
                    // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                    Err(error) => {
                        return Err((
                            CODE_INVALID_BEP,
                            format!("invalid change for output: {error}"),
                        ));
                    } // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                },
                // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                Err(reason) => {
                    return Err((
                        CODE_INVALID_BEP,
                        format!("invalid change for {}: {reason}", change.path),
                    ));
                } // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
            }
        }
        if !invocation.check {
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
                match mutation_event(
                    &change.path,
                    ChangeKind::Modify,
                    if is_applied {
                        MutationOutcome::Applied
                    } else {
                        MutationOutcome::NotApplied
                    },
                    reason,
                ) {
                    Ok(event) => {
                        if is_applied {
                            applied_count += 1;
                        } else {
                            not_applied_count += 1;
                        }
                        let _ = write_event(out, &event);
                    }
                    // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                    Err(error) => {
                        return Err((
                            CODE_INVALID_BEP,
                            format!("invalid mutation for output: {error}"),
                        ));
                    } // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                }
            }
        }
    } else if matches!(invocation.output, OutputMode::Text { .. }) {
        let human: &mut dyn Write = if stdout_report { err } else { out };
        for diagnostic in status {
            let _ = writeln!(human, "{}", text_diagnostic(diagnostic));
        }
        if !invocation.check {
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
    Ok(EmitCounts {
        applied_count,
        not_applied_count,
    })
}
