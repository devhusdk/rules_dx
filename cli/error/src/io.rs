//! A filesystem error that keeps the path and the `std::io::Error` it came from.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoOp {
    Read,
    Write,
    Create,
    CreateDir,
    ReadDir,
    Remove,
    Copy,
    Rename,
    Open,
    Stat,
    Canonicalize,
}

impl IoOp {
    const fn verb(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Create => "create",
            Self::CreateDir => "create directory",
            Self::ReadDir => "list",
            Self::Remove => "remove",
            Self::Copy => "copy",
            Self::Rename => "rename",
            Self::Open => "open",
            Self::Stat => "stat",
            Self::Canonicalize => "resolve",
        }
    }
}

/// Displays as `cannot <op> <path>: <source>`; `source()` returns the io error.
#[derive(Debug)]
pub struct IoError {
    op: IoOp,
    path: PathBuf,
    source: io::Error,
}

impl IoError {
    pub fn new(op: IoOp, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self {
            op,
            path: path.into(),
            source,
        }
    }

    #[must_use]
    pub const fn op(&self) -> IoOp {
        self.op
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn kind(&self) -> io::ErrorKind {
        self.source.kind()
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot {} {}: {}",
            self.op.verb(),
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Attaches the operation and path to an `io::Result`.
pub trait IoContext<T> {
    fn io_context(self, op: IoOp, path: impl AsRef<Path>) -> Result<T, IoError>;

    fn read_context(self, path: impl AsRef<Path>) -> Result<T, IoError>
    where
        Self: Sized,
    {
        self.io_context(IoOp::Read, path)
    }

    fn write_context(self, path: impl AsRef<Path>) -> Result<T, IoError>
    where
        Self: Sized,
    {
        self.io_context(IoOp::Write, path)
    }
}

impl<T> IoContext<T> for io::Result<T> {
    fn io_context(self, op: IoOp, path: impl AsRef<Path>) -> Result<T, IoError> {
        self.map_err(|source| IoError::new(op, path.as_ref(), source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn message_keeps_op_path_and_source() {
        let result: io::Result<()> = Err(io::Error::new(io::ErrorKind::NotFound, "gone"));
        let error = result.read_context("a/b.json").unwrap_err();
        assert_eq!(error.to_string(), "cannot read a/b.json: gone");
        assert_eq!(error.op(), IoOp::Read);
        assert_eq!(error.path(), Path::new("a/b.json"));
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(error.source().map(ToString::to_string).as_deref(), Some("gone"));
    }

    #[test]
    fn every_op_names_its_verb() {
        for (op, verb) in [
            (IoOp::Write, "write"),
            (IoOp::Create, "create"),
            (IoOp::CreateDir, "create directory"),
            (IoOp::ReadDir, "list"),
            (IoOp::Remove, "remove"),
            (IoOp::Copy, "copy"),
            (IoOp::Rename, "rename"),
            (IoOp::Open, "open"),
            (IoOp::Stat, "stat"),
            (IoOp::Canonicalize, "resolve"),
        ] {
            let error = IoError::new(op, "p", io::Error::other("e"));
            assert_eq!(error.to_string(), format!("cannot {verb} p: e"));
        }
        let ok: io::Result<u8> = Ok(1);
        assert_eq!(ok.write_context("p").ok(), Some(1));
    }
}
