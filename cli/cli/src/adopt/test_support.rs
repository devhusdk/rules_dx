use super::{execute_adoption, AdoptEnv, Invocation};
use crate::resolve::{QueryResult, QueryRunner};
use dx_process::{ChildStatus, Runner};
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) use crate::args::parsed as invocation;
pub(crate) use crate::test_support::{event, event_kinds, events_of_kind, json_events};

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

/// A runner that launches nothing, reports success, and selects a hermetic Git.
pub(crate) struct HermeticRunner;

impl Runner for HermeticRunner {
    fn run(&self, _argv: &[String], _cwd: &Path, _env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        Ok(ChildStatus { code: Some(0) })
    }

    fn git_tool(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/hermetic/git"))
    }
}

/// A query runner that answers every call with the same recorded output.
pub(crate) struct RepeatQuery {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    seen: RefCell<Vec<Vec<String>>>,
}

impl RepeatQuery {
    /// Answers every call with one hooks dir discovery line.
    pub(crate) fn hooks_dir(dir: &str) -> Self {
        Self::result(Some(0), &format!("{dir}\n"), "")
    }

    /// Answers every call with the same code and streams.
    pub(crate) fn result(code: Option<i32>, stdout: &str, stderr: &str) -> Self {
        Self {
            code,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            seen: RefCell::new(Vec::new()),
        }
    }

    /// Reports every argv the runner received, in order.
    pub(crate) fn calls(&self) -> Vec<Vec<String>> {
        self.seen.borrow().clone()
    }
}

impl QueryRunner for RepeatQuery {
    fn run_query(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        self.seen.borrow_mut().push(argv.to_vec());
        Ok(QueryResult {
            code: self.code,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
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

/// Runs one adoption against the null runners and reports code, stdout and stderr.
pub(crate) fn run(inv: &Invocation, root: &Path) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute_adoption(inv, env(root, &mut out, &mut err));
    spoken(code, out, err)
}

/// The same, with a caller-chosen query runner.
pub(crate) fn run_with_query(
    inv: &Invocation,
    root: &Path,
    query: &dyn QueryRunner,
) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute_adoption(inv, env_with_query(root, query, &mut out, &mut err));
    spoken(code, out, err)
}

/// The same, with a caller-chosen query runner and process runner.
pub(crate) fn run_with(
    inv: &Invocation,
    root: &Path,
    query: &dyn QueryRunner,
    runner: &dyn Runner,
) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute_adoption(inv, env_with(root, query, runner, &mut out, &mut err));
    spoken(code, out, err)
}

/// Reads one run's exit code and both streams back as text.
fn spoken(code: i32, out: Vec<u8>, err: Vec<u8>) -> (i32, String, String) {
    (
        code,
        String::from_utf8(out).expect("stdout"),
        String::from_utf8(err).expect("stderr"),
    )
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
    use dx_process::pre_exec_code;

    /// A query runner that answers every call with the same targets.
    struct FixedQuery(&'static str);

    impl QueryRunner for FixedQuery {
        fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Ok(QueryResult {
                code: Some(0),
                stdout: self.0.as_bytes().to_vec(),
                stderr: Vec::new(),
            })
        }
    }

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
    fn run_reports_the_code_and_what_reached_each_stream() {
        let scratch = dx_test_scratch::scratch("dx-adopt-run-streams-");
        let (code, out, err) = run(&invocation(&["init", "--dry-run"]), scratch.path());
        assert_eq!(code, 0);
        assert!(out.contains(".dx/version"), "{out}");
        assert!(err.is_empty(), "{err}");

        let scratch = dx_test_scratch::scratch("dx-adopt-run-refused-");
        let (code, out, err) = run(&invocation(&["new", "ruby", "demo"]), scratch.path());
        assert_eq!(code, pre_exec_code());
        assert!(out.is_empty(), "{out}");
        assert!(err.contains("unknown language"), "{err}");
    }

    #[test]
    fn run_with_query_answers_from_the_query_it_is_given() {
        let scratch = dx_test_scratch::scratch("dx-adopt-run-query-");
        let (code, out, err) = run_with_query(
            &invocation(&["owners", "//a:one"]),
            scratch.path(),
            &FixedQuery("//z:other\n"),
        );
        assert_eq!(code, 0);
        assert_eq!(out, "//z:other\n");
        assert!(err.is_empty(), "{err}");

        let (code, out, err) = run_with_query(
            &invocation(&["why", "src/lib.rs", "//app:server"]),
            scratch.path(),
            &FixedQuery(""),
        );
        assert_eq!(code, 1);
        assert!(!out.contains("//app:server"), "{out}");
        assert!(err.contains("no owner"), "{err}");
    }

    #[test]
    fn run_with_answers_from_the_query_and_the_runner_it_is_given() {
        let scratch = dx_test_scratch::scratch("dx-adopt-run-with-");
        let inv = invocation(&["why", "src/lib.rs", "//app:server"]);
        let (code, _out, err) = run_with(
            &inv,
            scratch.path(),
            &FixedQuery("//owner:lib\n"),
            &NullRunner,
        );
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");

        let (code, _out, err) = run_with(&inv, scratch.path(), &FixedQuery(""), &NullRunner);
        assert_eq!(code, 1);
        assert!(err.contains("no owner"), "{err}");
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
