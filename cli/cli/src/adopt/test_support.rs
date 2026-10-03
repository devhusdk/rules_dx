use super::AdoptEnv;
use crate::resolve::{QueryResult, QueryRunner};
use dx_process::{ChildStatus, Runner};
use std::io::{self, Write};
use std::path::Path;

pub(crate) use crate::args::parsed as invocation;

/// A query runner that reports one target and nothing else.
pub(crate) struct NullQuery;

impl QueryRunner for NullQuery {
    fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        Ok(QueryResult {
            code: Some(0),
            stdout: b"//a:one\n".to_vec(),
            stderr: Vec::new(),
        })
    }
}

/// A runner that launches nothing and reports success.
pub(crate) struct NullRunner;

impl Runner for NullRunner {
    fn run(&self, _argv: &[String], _cwd: &Path, _env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        Ok(ChildStatus { code: Some(0) })
    }
}

/// The adoption environment wired to the null query runner.
pub(crate) fn env<'a>(
    root: &'a Path,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
) -> AdoptEnv<'a> {
    AdoptEnv {
        workspace: root,
        query_runner: &NullQuery,
        runner: &NullRunner,
        out,
        err,
    }
}

/// The same, with a caller-chosen query runner.
pub(crate) fn env_with_query<'a>(
    root: &'a Path,
    query: &'a dyn QueryRunner,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
) -> AdoptEnv<'a> {
    AdoptEnv {
        workspace: root,
        query_runner: query,
        runner: &NullRunner,
        out,
        err,
    }
}

/// The same, with a caller-chosen query runner and process runner.
pub(crate) fn env_with<'a>(
    root: &'a Path,
    query: &'a dyn QueryRunner,
    runner: &'a dyn Runner,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
) -> AdoptEnv<'a> {
    AdoptEnv {
        workspace: root,
        query_runner: query,
        runner,
        out,
        err,
    }
}

/// Counts the budget of a truncated writer.
#[derive(Clone, Copy)]
enum Unit {
    Bytes,
    Lines,
}

/// Writes a budget of bytes or of newlines, then reports a broken pipe.
pub(crate) struct Truncated {
    remaining: usize,
    unit: Unit,
}

impl Truncated {
    /// Accepts `bytes` more bytes, then breaks.
    pub(crate) fn after_bytes(bytes: usize) -> Self {
        Self {
            remaining: bytes,
            unit: Unit::Bytes,
        }
    }

    /// Accepts `lines` more newlines, then breaks.
    pub(crate) fn after_lines(lines: usize) -> Self {
        Self {
            remaining: lines,
            unit: Unit::Lines,
        }
    }
}

impl Write for Truncated {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"));
        }
        let spent = match self.unit {
            Unit::Bytes => self.remaining.min(bytes.len()),
            Unit::Lines => bytes.iter().filter(|byte| **byte == b'\n').count(),
        };
        self.remaining = self.remaining.saturating_sub(spent);
        Ok(match self.unit {
            Unit::Bytes => spent,
            Unit::Lines => bytes.len(),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(result: io::Result<usize>) -> Result<usize, io::ErrorKind> {
        result.map_err(|error| error.kind())
    }

    #[test]
    fn null_query_reports_one_target_and_success() {
        let result = NullQuery.run_query(&[], Path::new("/")).expect("query");
        assert_eq!(result.code, Some(0));
        assert_eq!(result.stdout, b"//a:one\n");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn null_runner_launches_nothing_and_succeeds() {
        let status = NullRunner.run(&[], Path::new("/"), &[]).expect("launch");
        assert_eq!(status.code, Some(0));
    }

    #[test]
    fn truncated_spends_its_budget_in_bytes_and_in_lines() {
        let mut bytes = Truncated::after_bytes(4);
        assert_eq!(kind(bytes.write(b"abcdef")), Ok(4));
        assert_eq!(kind(bytes.write(b"g")), Err(io::ErrorKind::BrokenPipe));

        let mut lines = Truncated::after_lines(2);
        assert_eq!(kind(lines.write(b"abc\n")), Ok(4));
        assert_eq!(kind(lines.write(b"d\n")), Ok(2));
        assert_eq!(kind(lines.write(b"e\n")), Err(io::ErrorKind::BrokenPipe));

        let mut multi = Truncated::after_lines(1);
        assert_eq!(kind(multi.write(b"a\nb\nc\n")), Ok(6));
        assert_eq!(kind(multi.write(b"d\n")), Err(io::ErrorKind::BrokenPipe));

        let mut none = Truncated::after_bytes(0);
        assert_eq!(kind(none.write(b"a")), Err(io::ErrorKind::BrokenPipe));
        assert!(none.flush().is_ok());
    }
}
