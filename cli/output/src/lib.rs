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
    change_event, change_value, mutation_event, mutation_value, ChangeEvent, ChangeKind,
    MutationOutcome,
};
pub use clap_errors::{first_line, missing_value_flag, parse_error, rejected_value, unknown_token};
pub use diagnostics::{
    color_enabled, color_enabled_for, color_override, colors_allowed, colors_allowed_for,
    first_diagnostic_line, init_diagnostics, init_diagnostics_with_color,
    init_diagnostics_with_level, resolve_log_filter, set_color_override, truncate_line, ColorMode,
    LogLevel, DEFAULT_LOG_FILTER, VERBOSE_LOG_FILTER,
};
pub use findings::{
    diagnostic_event, diagnostic_value, notice_event, notice_value, sort_diagnostics,
    DiagnosticEvent, NoticeEvent, Resolution, Snapshot,
};
pub use lifecycle::{
    capabilities_event, command_finished, command_started, error_event, operation_event,
    report_event, schema, selection_event, status_event, test_outcome_event, with_correlation,
    write_event, CapabilitiesEntry, FinishedCounts, StatusEvent, TestOutcome, EVENTS, SCHEMA_MAJOR,
    SCHEMA_MINOR,
};
pub use modes::{
    check_output_conflict, dx_text_visible, stdout_owner, OutputMode, OutputModeName, StdoutOwner,
};
pub use severity::{meets_threshold, Severity, Threshold};
pub use validation::{check_correlation, check_edits, check_path, parse_digest, Edit, OutputError};
