// LCOV_EXCL_START - reason: re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod changes;
pub mod clap_errors;
pub mod diagnostics;
pub mod findings;
pub mod lifecycle;
pub mod modes;
pub mod severity;
pub mod validation;

pub use changes::{
    ChangeEffect, ChangeEvent, ChangeKind, change_event, change_value, refusal_value,
};
pub use clap_errors::{first_line, missing_value_flag, parse_error, rejected_value, unknown_token};
pub use diagnostics::{
    ColorMode, DEFAULT_LOG_FILTER, Excerpt, VERBOSE_LOG_FILTER, color_enabled, color_enabled_for,
    color_override, colors_allowed, colors_allowed_for, diagnostic_excerpt, first_diagnostic_line,
    init_diagnostics, init_diagnostics_with_color, set_color_override, truncate_line,
};
pub use findings::{
    DiagnosticEvent, FindingRecord, NoticeEvent, Origin, PackageRef, Position, RelatedEvent,
    Resolution, Snapshot, Span, diagnostic_event, finding_value, notice_event, notice_value,
    sort_diagnostics,
};
pub use lifecycle::{
    ActionCounts, BuildFailureEvent, CapabilityEvent, ChangedLineCoverageEvent, CoverageEvent,
    EVENT_MESSAGE_LIMIT, EVENTS, FAILING_CASE_LIMIT, FailureDiagnostic, FailureFacts,
    FailureRelated, FinishedCounts, OwnedFileEvent, PlanFindingEvent, SCHEMA_MAJOR, SCHEMA_MINOR,
    SlotEvent, StatusEvent, SummaryEvent, TestCase, TestCounts, TestOutcome, build_failure_event,
    capability_event, capability_value, changed_line_coverage_event, command_finished,
    command_started, coverage_event, error_event, failure_event, init_plan_event, nest_phase_event,
    operation_event, owned_file_event, quality_started, read_event, report_event, schema,
    selection_event, slot_event, status_event, summary_event, test_outcome_event,
    test_result_record, with_correlation, write_event,
};
pub use modes::{
    OutputMode, OutputModeName, StdoutOwner, check_output_conflict, dx_text_visible, stdout_owner,
};
pub use severity::{Severity, Threshold, meets_threshold};
pub use validation::{Edit, OutputError, check_correlation, check_edits, check_path, parse_digest};

#[cfg(test)]
mod schema_tests;
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
