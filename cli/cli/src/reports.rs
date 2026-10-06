pub mod junit;
mod junit_parse;
mod junit_render;
mod junit_types;
pub mod lcov;
pub mod planning;
pub mod sarif;

use std::path::Path;

pub use junit::{junit_infrastructure_case, parse_test_xml, render_junit, JunitCase, JunitMessage};
pub use lcov::{coverage_line_rate, validate_lcov};
pub use planning::{
    format_names, plan_reports, resolve_report_path, Destination, PlannedReport, ReportPath,
    StandardFormat,
};
pub use sarif::{byte_to_line, render_sarif};

fn unsupported_format_message(command: &str, format: &str, supported: &[&str]) -> String {
    if supported.is_empty() {
        format!("unsupported report format {format:?} for {command}: no standard report exists")
    } else {
        format!(
            "unsupported report format {format:?} for {command}: want {}",
            supported.join("|")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportError {
    #[error("--dry-run conflicts with every --report request")]
    DryRunConflict,
    #[error(
        "{msg}",
        msg = unsupported_format_message(command, format, supported)
    )]
    UnsupportedFormat {
        command: &'static str,
        format: String,
        supported: Vec<&'static str>,
    },
    #[error("duplicate report {format:?} for destination {destination:?}")]
    DuplicateReport { format: String, destination: String },
    #[error(
        "report destinations {first:?} and {second:?} both resolve to {resolved:?}: give each report its own file"
    )]
    DestinationCollision {
        first: String,
        second: String,
        resolved: String,
    },
    #[error("more than one standard report targets stdout")]
    MultipleStdoutReports,
    #[error(
        "a stdout report conflicts with --output {mode}: use --output text or a file destination"
    )]
    StdoutReportConflictsMode { mode: &'static str },
    #[error("invalid finding for SARIF export: {detail}")]
    InvalidFinding { detail: &'static str },
    #[error("a byte range without a path cannot be located")]
    RangeWithoutPath,
    #[error("a byte range starts after its end")]
    InvertedRange,
    #[error("missing source snapshot for ranged finding in {path:?}")]
    MissingSnapshot { path: String },
    #[error("finding references unknown tool {tool:?}")]
    UnknownTool { tool: String },
    #[error("byte offset {offset} is not a character boundary in {path:?}")]
    BadOffset { path: String, offset: u64 },
    #[error("invalid Bazel test XML artifact: {detail}")]
    InvalidJunit { detail: String },
    #[error("invalid Bazel combined LCOV tracefile: {detail}")]
    InvalidLcov { detail: String },
    #[error("unexpected output conflict: {detail}")]
    UnexpectedOutputConflict { detail: String },
    #[error("{0}")]
    Fingerprint(#[from] dx_fingerprint::FingerprintError),
    #[error("junit report serialization failed: {detail}")]
    JunitRender { detail: String },
}

/// Why one report document did not land at its resolved destination.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportWriteError {
    #[error("failed to write {format} report to {destination}: {cause}")]
    Write {
        format: &'static str,
        destination: String,
        cause: ReportWriteCause,
    },
}

/// The filesystem reason behind one failed report write.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportWriteCause {
    #[error("parent directory {0} does not exist")]
    MissingParent(String),
    #[error("parent {0} is not a directory")]
    ParentNotDirectory(String),
    #[error("{0}")]
    Io(String),
}

impl ReportWriteError {
    pub fn new(
        format: &'static str,
        destination: &Path,
        cause: ReportWriteCause,
    ) -> ReportWriteError {
        ReportWriteError::Write {
            format,
            destination: destination.display().to_string(),
            cause,
        }
    }
}
