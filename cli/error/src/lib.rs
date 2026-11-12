//! The error type every dx command returns and the one table that maps it to
//! a process exit code.

use std::fmt;
use std::process::ExitCode;

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_INFRA: i32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Usage,
    Failure,
    Infra,
}

impl ErrorKind {
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Usage => EXIT_USAGE,
            Self::Failure => EXIT_FAILURE,
            Self::Infra => EXIT_INFRA,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DxError {
    kind: ErrorKind,
    message: String,
}

impl DxError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, message)
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Failure, message)
    }

    pub fn infra(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Infra, message)
    }

    pub fn from_io(message: impl Into<String>, error: &std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::InvalidData => Self::failure(message),
            _ => Self::infra(message),
        }
    }

    #[must_use]
    pub fn with_context(self, context: impl fmt::Display) -> Self {
        Self {
            kind: self.kind,
            message: format!("{context}: {}", self.message),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        self.kind.exit_code()
    }
}

impl fmt::Display for DxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DxError {}

#[must_use]
pub fn exit_code(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

impl From<ErrorKind> for ExitCode {
    fn from(kind: ErrorKind) -> Self {
        exit_code(kind.exit_code())
    }
}

impl From<DxError> for ExitCode {
    fn from(error: DxError) -> Self {
        exit_code(error.exit_code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_map_to_the_documented_codes() {
        assert_eq!(ErrorKind::Failure.exit_code(), 1);
        assert_eq!(ErrorKind::Usage.exit_code(), 2);
        assert_eq!(ErrorKind::Infra.exit_code(), 3);
    }

    #[test]
    fn constructors_set_kind_and_message() {
        let error = DxError::usage("bad flag");
        assert_eq!(error.kind(), ErrorKind::Usage);
        assert_eq!(error.message(), "bad flag");
        assert_eq!(error.to_string(), "bad flag");
        assert_eq!(error.exit_code(), EXIT_USAGE);
        assert_eq!(DxError::failure("x").exit_code(), EXIT_FAILURE);
        assert_eq!(DxError::infra("x").exit_code(), EXIT_INFRA);
    }

    #[test]
    fn with_context_prefixes_the_message_and_keeps_the_kind() {
        let error = DxError::infra("disk full").with_context("failed to write");
        assert_eq!(error.kind(), ErrorKind::Infra);
        assert_eq!(error.message(), "failed to write: disk full");
    }

    #[test]
    fn from_io_classifies_by_io_kind() {
        use std::io::{Error, ErrorKind as Io};
        for (kind, expected) in [
            (Io::NotFound, ErrorKind::Failure),
            (Io::InvalidInput, ErrorKind::Failure),
            (Io::InvalidData, ErrorKind::Failure),
            (Io::PermissionDenied, ErrorKind::Infra),
            (Io::IsADirectory, ErrorKind::Infra),
            (Io::Other, ErrorKind::Infra),
        ] {
            let error = DxError::from_io("m", &Error::from(kind));
            assert_eq!(error.kind(), expected, "{kind:?}");
            assert_eq!(error.message(), "m");
        }
    }

    #[test]
    fn exit_code_falls_back_to_failure_outside_u8() {
        assert_eq!(exit_code(0), ExitCode::from(0));
        assert_eq!(exit_code(2), ExitCode::from(2));
        assert_eq!(exit_code(3), ExitCode::from(3));
        assert_eq!(exit_code(300), ExitCode::from(1));
        assert_eq!(exit_code(-1), ExitCode::from(1));
        assert_eq!(ExitCode::from(ErrorKind::Usage), ExitCode::from(2));
    }

    #[test]
    fn converts_to_process_exit_code() {
        assert_eq!(ExitCode::from(DxError::usage("x")), ExitCode::from(2));
    }
}
