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
    change_event, change_value, refusal_event, refusal_value, ChangeEffect, ChangeEvent, ChangeKind,
};
pub use clap_errors::{first_line, missing_value_flag, parse_error, rejected_value, unknown_token};
pub use diagnostics::{
    color_enabled, color_enabled_for, color_override, colors_allowed, colors_allowed_for,
    diagnostic_excerpt, first_diagnostic_line, init_diagnostics, init_diagnostics_with_color,
    set_color_override, truncate_line, ColorMode, Excerpt, DEFAULT_LOG_FILTER, VERBOSE_LOG_FILTER,
};
pub use findings::{
    diagnostic_event, diagnostic_value, finding_value, notice_event, notice_value,
    sort_diagnostics, DiagnosticEvent, FindingRecord, NoticeEvent, Origin, PackageRef, Position,
    RelatedEvent, Resolution, Snapshot, Span,
};
pub use lifecycle::{
    build_failure_event, capability_event, capability_value, changed_line_coverage_event,
    command_finished, command_started, coverage_event, error_event, failure_event,
    nest_phase_event, operation_event, read_event, report_event, schema, selection_event,
    slot_event, status_event, summary_event, test_outcome_event, test_result_record,
    with_correlation, write_event, BuildFailureEvent, CapabilityEvent, ChangedLineCoverageEvent,
    CoverageEvent, FailureDiagnostic, FailureFacts, FailureRelated, FinishedCounts, SlotEvent,
    StatusEvent, SummaryEvent, TestCase, TestCounts, TestOutcome, EVENTS, EVENT_MESSAGE_LIMIT,
    FAILING_CASE_LIMIT, SCHEMA_MAJOR, SCHEMA_MINOR,
};
pub use modes::{
    check_output_conflict, dx_text_visible, stdout_owner, OutputMode, OutputModeName, StdoutOwner,
};
pub use severity::{meets_threshold, Severity, Threshold};
pub use validation::{check_correlation, check_edits, check_path, parse_digest, Edit, OutputError};
