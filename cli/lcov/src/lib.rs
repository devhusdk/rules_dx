#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod ignores;
pub mod inventory;
pub mod parse;
pub mod run;
pub mod verdict;

pub use ignores::{find_ignores, is_ignored, Ignores};
pub use inventory::{parse_inventory, ELIGIBLE, SUPPORT};
pub use parse::{merge_lcov_reports, parse_lcov, validate_lcov_report, FileHits};
pub use run::run;
pub use verdict::{evaluate, is_covered_language, render, FileVerdict, GateVerdict};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LcovError {
    #[error("LCOV record with empty SF path")]
    EmptySfPath,
    #[error("LCOV DA record outside any SF record: {line}")]
    DaOutsideSf { line: String },
    #[error("malformed LCOV DA line number in {path}: {line}")]
    MalformedLineNumber { path: String, line: String },
    #[error("malformed LCOV DA hit count in {path}: {line}")]
    MalformedHitCount { path: String, line: String },
    #[error("LCOV SF record before end_of_record")]
    SfBeforeEndOfRecord,
    #[error("LCOV end_of_record outside any SF record")]
    EndOfRecordOutsideSf,
    #[error("LCOV has no SF records")]
    NoSfRecords,
    #[error("LCOV SF record without end_of_record")]
    SfWithoutEndOfRecord,
    #[error("coverage ignore without nearby reason at {path}:{lineno}: {directive} requires a reason: comment on the same or previous line")]
    MissingReason {
        path: String,
        lineno: usize,
        directive: String,
    },
    #[error("coverage ignore with bare policy at {path}:{lineno}: {directive} requires reason: plus issue: (bare policy: without reason: is rejected)")]
    BarePolicyWithoutReason {
        path: String,
        lineno: usize,
        directive: String,
    },
    #[error("coverage ignore without issue tracking at {path}:{lineno}: {directive} requires issue: <number> on the same or previous line for budget/expiry review")]
    MissingIssue {
        path: String,
        lineno: usize,
        directive: String,
    },
    #[error("coverage ignore reason too long at {path}:{lineno}: {directive} reason is {len} chars, max {max}")]
    ReasonTooLong {
        path: String,
        lineno: usize,
        directive: String,
        len: usize,
        max: usize,
    },
    #[error("nested range START at {path}:{lineno}")]
    NestedStart { path: String, lineno: usize },
    #[error("range STOP without START at {path}:{lineno}")]
    StopWithoutStart { path: String, lineno: usize },
    #[error("unrecognized coverage ignore directive at {path}:{lineno}")]
    UnrecognizedDirective { path: String, lineno: usize },
    #[error("unclosed range START at {path}:{start}")]
    UnclosedStart { path: String, start: usize },
    #[error("malformed inventory line {lineno}: {raw:?}")]
    MalformedInventory { lineno: usize, raw: String },
    #[error("{message}")]
    Io { message: String },
}

impl From<String> for LcovError {
    fn from(message: String) -> Self {
        Self::Io { message }
    }
}

#[cfg(test)]
mod facade_tests {
    use super::LcovError;

    #[test]
    fn string_conversion_preserves_message() {
        let error = LcovError::from("fixture io failure".to_string());
        assert_eq!(
            error,
            LcovError::Io {
                message: "fixture io failure".to_string()
            }
        );
        assert!(error.to_string().contains("fixture io failure"));
    }
}
