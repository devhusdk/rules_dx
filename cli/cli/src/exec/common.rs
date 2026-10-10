use crate::args::Invocation;
use crate::reports::Destination;
use crate::resolve::QueryRunner;
use dx_apply::{FileSystem, RealFileSystem};
use dx_bep::ArtifactReader;
use dx_digest::blake3 as digest;
use dx_output::{
    command_finished, command_started, write_event, ChangeEvent, ChangeKind, DiagnosticEvent,
    FinishedCounts, OutputMode,
};
use dx_process::{broken_pipe_code, operational_code, pre_exec_code, stdout_io_code, Runner};
use std::io::{self, BufReader, Write};
use std::path::Path;

pub(crate) fn stdout_output_code(error: &dx_output::OutputError) -> i32 {
    if error.is_broken_pipe() {
        broken_pipe_code()
    } else {
        operational_code()
    }
}

pub fn emit_event(out: &mut dyn Write, event: &serde_json::Value) -> Result<(), i32> {
    write_event(out, event).map_err(|error| stdout_output_code(&error))
}

pub(crate) fn check_stdout_write(result: io::Result<()>) -> Result<(), i32> {
    result.map_err(|error| stdout_io_code(&error))
}

const OFFLINE_MARKER: &str = " (offline, cache-only)";

/// Marks one summary line as cache-only when the invocation ran offline.
pub(crate) fn offline_summary(summary: String, offline: bool) -> String {
    if offline {
        format!("{summary}{OFFLINE_MARKER}")
    } else {
        summary
    }
}

const FROZEN_MARKER: &str = " (frozen, no resolution changes)";

/// Marks one summary line as resolution-locked when the invocation runs frozen.
pub(crate) fn frozen_summary(summary: String, frozen: bool) -> String {
    if frozen {
        format!("{summary}{FROZEN_MARKER}")
    } else {
        summary
    }
}

pub fn flush_out(out: &mut dyn Write) -> Result<(), i32> {
    out.flush().map_err(|error| stdout_io_code(&error))
}

pub(crate) fn emit_started(invocation: &Invocation, out: &mut dyn Write) -> Result<(), i32> {
    if let Ok(event) = command_started(invocation.command.name(), invocation.dry_run, "default") {
        emit_event(out, &event)?;
    }
    Ok(())
}

pub const REASON_STALE_SOURCE: &str = "stale_source";
pub const REASON_UNREADABLE_SOURCE: &str = "unreadable_source";
pub const REASON_INVALID_EDITS: &str = "invalid_edits";
pub const REASON_INCOMPLETE_COLLECTION: &str = "incomplete_collection";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ExecError {
    #[error("generated logical path is empty")]
    EmptyLogicalPath,
    #[error("generated logical path {path:?} is absolute")]
    AbsoluteLogicalPath { path: String },
    #[error("generated logical path {path:?} escapes its generation")]
    EscapingLogicalPath { path: String },
    #[error("env identity key is empty")]
    EmptyEnvKey,
    #[error("env identity key {key:?} is not a single filename")]
    BadEnvKey { key: String },
}

pub(crate) const CODE_LAUNCH_FAILED: &str = "launch_failed";
pub(crate) const CODE_BAZEL_SIGNALLED: &str = "bazel_signalled";
pub(crate) const CODE_UNREADABLE_BEP: &str = "unreadable_bep";
pub(crate) const CODE_INVALID_BEP: &str = "invalid_bep";
pub(crate) const CODE_DIFF_FAILED: &str = "diff_failed";
pub(crate) const CODE_REPORT_FAILED: &str = "report_failed";
pub(crate) const CODE_COLLECTION_FAILED: &str = "collection_failed";
pub(crate) const CODE_VERIFICATION_FAILED: &str = "verification_failed";
pub(crate) const CODE_COVERAGE_BELOW_MINIMUM: &str = "coverage_below_minimum";
pub(crate) const CODE_CLEAN_FAILED: &str = "clean_failed";
pub(crate) const CODE_INVALID_RESULT: &str = "invalid_result";
pub(crate) const CODE_MANAGED_COMMIT_FAILED: &str = "managed_commit_failed";
pub(crate) const CODE_MANAGED_NO_CAPABILITY: &str = "no_capability";
pub(crate) const CODE_AUDIT_FAILED: &str = "audit_failed";
pub(crate) const CODE_UPDATE_FAILED: &str = "update_failed";
pub(crate) const CODE_BUMP_FAILED: &str = "bump_failed";
pub(crate) const CODE_MIGRATE_FAILED: &str = "migrate_failed";
pub(crate) const CODE_OFFLINE_REQUIRED: &str = "offline_required";
pub(crate) const CODE_FROZEN_LOCKED: &str = "frozen_locked";

pub struct Env<'a> {
    pub workspace: &'a Path,
    pub runner: &'a dyn Runner,
    pub query_runner: &'a dyn QueryRunner,
    pub temp_dir: &'a Path,
    pub pid: u32,
    pub nonce: u64,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub ci: bool,
}

pub(crate) struct FsArtifacts;

impl ArtifactReader for FsArtifacts {
    fn read_artifact(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
}

/// Reads one build event file and returns the target outputs of one output group.
pub(crate) fn collect_targets(
    bep: &Path,
    group: &str,
    workspace: &Path,
) -> Result<Vec<dx_bep::TargetOutput>, (String, String)> {
    let file = std::fs::File::open(bep).map_err(|err| {
        (
            CODE_UNREADABLE_BEP.to_owned(),
            format!("failed to read build events: {err}"),
        )
    })?;
    let config = dx_bep::CollectorConfig::new(group).map_err(|err| {
        (
            CODE_INVALID_BEP.to_owned(),
            format!("invalid BEP config: {err}"),
        )
    })?;
    dx_bep::collect_with_workspace(BufReader::new(file), &config, &FsArtifacts, Some(workspace))
        .map_err(|err| {
            (
                CODE_INVALID_BEP.to_owned(),
                format!("invalid build events: {err}"),
            )
        })
}

pub(crate) struct FileChange {
    pub(crate) path: String,
    pub(crate) original_digest: [u8; 32],
    pub(crate) edits: Vec<(u64, u64, String)>,
}

pub(crate) fn hex_digest(bytes: &[u8; 32]) -> String {
    dx_digest::to_hex(bytes)
}

pub(crate) enum SourceRead {
    Bytes(Vec<u8>),
    Unreadable,
    Stale,
}

pub(crate) fn read_verified(workspace: &Path, path: &str, expected: &[u8; 32]) -> SourceRead {
    match std::fs::read(workspace.join(path)) {
        Err(_) => SourceRead::Unreadable,
        Ok(bytes) => {
            if digest(&bytes) == *expected {
                SourceRead::Bytes(bytes)
            } else {
                SourceRead::Stale
            }
        }
    }
}

pub(crate) fn apply_to_bytes(original: &[u8], edits: &[(u64, u64, String)]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(original).ok()?;
    let mut candidate = Vec::with_capacity(original.len());
    let mut cursor = 0usize;
    for (start, end, replacement) in edits {
        let (start, end) = (*start as usize, *end as usize);
        if start < cursor || start > end || end > original.len() {
            return None;
        }
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return None;
        }
        let head = &original[cursor..start];
        if std::str::from_utf8(head).is_err() {
            return None; // LCOV_EXCL_LINE - reason: utf8 slice of valid text, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        }
        candidate.extend_from_slice(head);
        candidate.extend_from_slice(replacement.as_bytes());
        cursor = end;
    }
    let tail = &original[cursor..];
    if std::str::from_utf8(tail).is_err() {
        return None; // LCOV_EXCL_LINE - reason: utf8 slice of valid text, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    }
    candidate.extend_from_slice(tail);
    Some(candidate)
}

pub(crate) fn text_diagnostic(diagnostic: &DiagnosticEvent) -> String {
    let mut line = format!("{} ", diagnostic.severity.name());
    if let Some(path) = &diagnostic.path {
        line.push_str(path);
        line.push_str(": ");
    }
    line.push_str(&diagnostic.message);
    line.push_str(" [");
    line.push_str(&diagnostic.tool);
    if let Some(rule) = &diagnostic.rule {
        line.push('/');
        line.push_str(rule);
    }
    line.push(']');
    if diagnostic.fixable {
        line.push_str(" (fixable)");
    }
    line
}

pub(crate) fn pre_exec(err: &mut dyn Write, message: &str) -> i32 {
    let _ = writeln!(err, "dx: {message}");
    let _ = writeln!(err, "{}", crate::args::help::usage_banner());
    pre_exec_code()
}

/// Reports one failed report as a `dx: report_failed:` line and, in JSON mode, an error event.
pub(crate) fn report_failed(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    detail: &str,
) {
    let _ = writeln!(err, "dx: {CODE_REPORT_FAILED}: {detail}");
    if output == OutputMode::Json {
        if let Ok(event) = dx_output::error_event(CODE_REPORT_FAILED, detail, None, None, None) {
            let _ = write_event(out, &event);
        }
    }
}

/// Writes one report document to its resolved destination and reports whether it landed.
pub(crate) fn write_report_file(
    workspace: &Path,
    destination: &str,
    document: &str,
) -> Result<(), ReportWriteError> {
    let target = crate::reports::resolve_destination(workspace, destination);
    let display = target.display().to_string();
    if target.is_dir() {
        return Err(ReportWriteError {
            destination: display,
            reason: "destination is a directory".to_owned(),
        });
    }
    let parent_missing = target
        .parent()
        .is_none_or(|parent| parent.as_os_str().is_empty() || !parent.is_dir());
    if parent_missing {
        let parent = target
            .parent()
            .map(|parent| parent.display().to_string())
            .unwrap_or_default();
        return Err(ReportWriteError {
            destination: display,
            reason: format!("parent directory {parent:?} does not exist"),
        });
    }
    RealFileSystem
        .write_atomic(&target, document.as_bytes())
        .map_err(|error| ReportWriteError {
            destination: display,
            reason: error.to_string(),
        })
}

/// Writes one report document to its planned destination and reports whether it landed.
pub(crate) fn write_report_document(
    out: &mut dyn Write,
    workspace: &Path,
    destination: &Destination,
    document: &str,
) -> Result<(), ReportWriteError> {
    match destination {
        Destination::Stdout => out
            .write_all(document.as_bytes())
            .and_then(|()| out.write_all(b"\n"))
            .map_err(|error| ReportWriteError {
                destination: "-".to_owned(),
                reason: error.to_string(),
            }),
        Destination::File(path) => write_report_file(workspace, path, document),
    }
}

/// Names the resolved report destination and why its write failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReportWriteError {
    pub(crate) destination: String,
    pub(crate) reason: String,
}

impl std::fmt::Display for ReportWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.destination, self.reason)
    }
}

pub(crate) fn operational(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    message: &str,
) -> i32 {
    let _ = writeln!(err, "dx: {code}: {message}");
    if invocation.output == OutputMode::Json {
        if let Ok(event) = dx_output::error_event(code, message, None, None, None) {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
        let finished = command_finished(
            operational_code(),
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return exit;
        }
    }
    operational_code()
}

/// Emits the terminal pair for a failure before the command starts: one
/// `error` event carrying the machine-readable code, then `command_finished`
/// with the process exit and `results_complete: false`. No `command_started`
/// precedes them because the command never began execution.
pub fn emit_startup_outcome(
    out: &mut dyn Write,
    code: &str,
    message: &str,
    exit: i32,
) -> Result<(), i32> {
    if let Ok(event) = dx_output::error_event(code, message, None, None, None) {
        emit_event(out, &event)?;
    }
    emit_event(
        out,
        &command_finished(
            exit,
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        ),
    )?;
    flush_out(out)
}

/// Runs one Bazel argv and maps a spawn failure and a signalled Bazel to operational exits.
pub(crate) fn run_bazel(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    workspace: &Path,
    runner: &dyn Runner,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<i32, i32> {
    let status = match runner.run(argv, workspace, env) {
        Ok(status) => status,
        Err(error) => {
            return Err(operational(
                invocation,
                out,
                err,
                CODE_LAUNCH_FAILED,
                &format!("failed to launch Bazel: {error}"),
            ));
        }
    };
    match status.code {
        Some(code) => Ok(code),
        None => Err(operational(
            invocation,
            out,
            err,
            CODE_BAZEL_SIGNALLED,
            "Bazel terminated by signal",
        )),
    }
}

pub(crate) fn change_event_for(change: &FileChange) -> ChangeEvent {
    let mut edits = Vec::with_capacity(change.edits.len());
    for (start, end, replacement) in &change.edits {
        edits.push(dx_output::Edit {
            start: *start,
            end: *end,
            replacement: replacement.clone(),
        });
    }
    ChangeEvent {
        path: change.path.clone(),
        kind: ChangeKind::Modify,
        source_digest: Some(hex_digest(&change.original_digest)),
        edits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_output::{Severity, Snapshot};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct ProbeRunner {
        code: Option<i32>,
        spawn_error: bool,
        seen: Rc<RefCell<(Vec<String>, Vec<(String, String)>)>>,
    }

    impl Runner for ProbeRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            if self.spawn_error {
                return Err(io::Error::other("fake launch failure"));
            }
            *self.seen.borrow_mut() = (
                argv.to_vec(),
                env.iter()
                    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                    .collect(),
            );
            Ok(dx_process::ChildStatus { code: self.code })
        }
    }

    fn bazel_invocation() -> Invocation {
        crate::args::parse(&["build".to_owned(), "//...".to_owned()]).expect("parse")
    }

    #[test]
    fn run_bazel_forwards_argv_and_env_and_returns_the_exit_code() {
        let seen = Rc::new(RefCell::new((Vec::new(), Vec::new())));
        let runner = ProbeRunner {
            code: Some(7),
            spawn_error: false,
            seen: Rc::clone(&seen),
        };
        let invocation = bazel_invocation();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let argv = vec!["bazel".to_owned(), "build".to_owned(), "//...".to_owned()];
        let code = run_bazel(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &argv,
            &[("DX_PROFILE", "release")],
        );
        assert_eq!(code, Ok(7));
        let seen = seen.borrow();
        assert_eq!(seen.0, argv);
        assert_eq!(
            seen.1,
            vec![("DX_PROFILE".to_owned(), "release".to_owned())]
        );
        assert_eq!(err, Vec::<u8>::new());
    }

    #[test]
    fn run_bazel_reports_a_spawn_failure_and_asks_the_caller_to_stop() {
        let runner = ProbeRunner {
            code: Some(0),
            spawn_error: true,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
        };
        let invocation = bazel_invocation();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_bazel(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &["bazel".to_owned()],
            &[],
        );
        assert_eq!(code, Err(operational_code()));
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "dx: launch_failed: failed to launch Bazel: fake launch failure\n"
        );
    }

    #[test]
    fn run_bazel_reports_a_signalled_bazel_and_asks_the_caller_to_stop() {
        let runner = ProbeRunner {
            code: None,
            spawn_error: false,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
        };
        let invocation = bazel_invocation();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_bazel(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &["bazel".to_owned()],
            &[],
        );
        assert_eq!(code, Err(operational_code()));
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "dx: bazel_signalled: Bazel terminated by signal\n"
        );
    }

    #[test]
    fn hex_digest_formats_lowercase_hex() {
        assert_eq!(hex_digest(&[0xabu8; 32]), "ab".repeat(32));
        assert_eq!(hex_digest(&[0u8; 32]).len(), 64);
    }

    #[test]
    fn apply_rejects_overlapping_edits_without_writing() {
        let original = b"abcdef";
        assert!(apply_to_bytes(original, &[(0, 2, "AB".to_owned())]).is_some());
        assert!(
            apply_to_bytes(original, &[(0, 2, "AB".to_owned()), (1, 3, "X".to_owned())]).is_none()
        );
        assert!(
            apply_to_bytes(original, &[(0, 6, "AB".to_owned()), (6, 7, "X".to_owned())]).is_none()
        );
        assert!(apply_to_bytes(b"\xff\xfe", &[(0, 1, "a".to_owned())]).is_none());
    }

    #[test]
    fn text_diagnostic_renders_identity() {
        let mut diagnostic = DiagnosticEvent {
            severity: Severity::Warning,
            tool: "lint-tool".to_owned(),
            message: "mapped".to_owned(),
            rule: Some("lint-tool/rule".to_owned()),
            path: Some("src/a.py".to_owned()),
            range: None,
            snapshot: Snapshot::Terminal,
            fixable: true,
            resolution: None,
        };
        assert_eq!(
            text_diagnostic(&diagnostic),
            "warning src/a.py: mapped [lint-tool/lint-tool/rule] (fixable)"
        );
        diagnostic.path = None;
        diagnostic.rule = None;
        diagnostic.fixable = false;
        assert_eq!(text_diagnostic(&diagnostic), "warning mapped [lint-tool]");
    }

    #[test]
    fn apply_rejects_boundary_and_encoding_violations() {
        assert!(apply_to_bytes("héllo".as_bytes(), &[(2, 3, "X".to_owned())]).is_none());
        assert!(apply_to_bytes("héllo".as_bytes(), &[(1, 2, "X".to_owned())]).is_none());
        assert!(apply_to_bytes(b"\xff", &[(0, 1, "X".to_owned())]).is_none());
        assert_eq!(
            apply_to_bytes(b"ab", &[(0, 1, "X".to_owned())]),
            Some(b"Xb".to_vec())
        );
    }

    #[test]
    fn change_event_is_deterministic_and_reconstructs_candidate() {
        let original = b"BAD\n";
        let terminal = "GOOD\n";
        let change = FileChange {
            path: "src/lib.rs".to_owned(),
            original_digest: digest(original),
            edits: vec![(0, original.len() as u64, terminal.to_owned())],
        };
        let first = change_event_for(&change);
        let second = change_event_for(&change);
        assert_eq!(first.path, "src/lib.rs");
        assert_eq!(first.path, second.path);
        assert_eq!(first.source_digest, second.source_digest);
        assert_eq!(first.source_digest, Some(hex_digest(&digest(original))));
        assert_eq!(first.edits.len(), 1);
        assert_eq!(first.edits[0].start, 0);
        assert_eq!(first.edits[0].end, original.len() as u64);
        assert_eq!(first.edits[0].replacement, terminal);
        assert_eq!(
            first, second,
            "the event must not depend on how many times it is built"
        );
        let planned = apply_to_bytes(original, &change.edits).expect("apply");
        assert_eq!(planned, terminal.as_bytes());
        let other = FileChange {
            path: "src/other.rs".to_owned(),
            original_digest: digest(original),
            edits: vec![(0, original.len() as u64, terminal.to_owned())],
        };
        let other_event = change_event_for(&other);
        assert_ne!(first.path, other_event.path);
    }

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Other, "boom"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Other, "boom"))
        }
    }

    #[test]
    fn stdout_helpers_map_broken_pipe_to_141() {
        assert_eq!(dx_process::broken_pipe_code(), 128 + 13);
        let broken = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
        assert_eq!(stdout_io_code(&broken), 128 + 13);
        let other = io::Error::new(io::ErrorKind::Other, "boom");
        assert_eq!(stdout_io_code(&other), operational_code());
        let broken_out = dx_output::OutputError::Io("Broken pipe (os error 32)".to_owned());
        assert!(broken_out.is_broken_pipe());
        assert_eq!(stdout_output_code(&broken_out), 128 + 13);
        let other_out = dx_output::OutputError::Io("boom".to_owned());
        assert!(!other_out.is_broken_pipe());
        assert_eq!(stdout_output_code(&other_out), operational_code());
    }

    #[test]
    fn emit_event_and_flush_fail_with_broken_pipe_code() {
        let event = command_finished(0, &FinishedCounts::default());
        assert_eq!(emit_event(&mut BrokenPipeWriter, &event), Err(128 + 13));
        assert_eq!(emit_event(&mut FailingWriter, &event), Err(1));
        assert_eq!(flush_out(&mut BrokenPipeWriter), Err(128 + 13));
        assert_eq!(flush_out(&mut FailingWriter), Err(1));
        assert!(check_stdout_write(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "broken pipe"
        )))
        .is_err());
    }

    #[test]
    fn report_failed_writes_the_line_and_only_json_emits_the_event() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        report_failed(
            &mut out,
            &mut err,
            OutputMode::Text { quiet: false },
            "boom",
        );
        assert_eq!(String::from_utf8(err).unwrap(), "dx: report_failed: boom\n");
        assert!(out.is_empty());

        let mut out = Vec::new();
        let mut err = Vec::new();
        report_failed(&mut out, &mut err, OutputMode::Diff, "boom");
        assert_eq!(String::from_utf8(err).unwrap(), "dx: report_failed: boom\n");
        assert!(out.is_empty());

        let mut out = Vec::new();
        let mut err = Vec::new();
        report_failed(&mut out, &mut err, OutputMode::Json, "boom");
        assert_eq!(String::from_utf8(err).unwrap(), "dx: report_failed: boom\n");
        let event: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(event["code"], CODE_REPORT_FAILED);
        assert_eq!(event["message"], "boom");
    }

    #[test]
    fn write_report_document_honours_the_destination() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut out = Vec::new();
        write_report_document(
            &mut out,
            dir.path(),
            &Destination::File("out.sarif".to_owned()),
            "{}\n",
        )
        .expect("write");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out.sarif")).expect("report"),
            "{}\n"
        );
        assert!(out.is_empty());

        write_report_document(&mut out, dir.path(), &Destination::Stdout, "{}").expect("stdout");
        assert_eq!(String::from_utf8(out).expect("utf8"), "{}\n");

        let err = write_report_document(
            &mut BrokenPipeWriter,
            dir.path(),
            &Destination::Stdout,
            "{}\n",
        )
        .expect_err("broken pipe");
        assert_eq!(err.destination, "-");
        assert!(err.reason.contains("broken pipe"), "{err}");
    }

    #[test]
    fn write_report_file_refuses_a_parent_that_is_not_a_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = write_report_file(dir.path(), "nested/out.sarif", "{}\n").expect_err("no parent");
        assert!(err.destination.contains("nested"), "{err}");
        assert!(err.destination.contains("out.sarif"), "{err}");
        assert!(err.reason.contains("parent directory"), "{err}");
        assert!(err.reason.contains("does not exist"), "{err}");
        assert!(!dir.path().join("nested").exists());
        std::fs::create_dir(dir.path().join("nested")).expect("mkdir");
        write_report_file(dir.path(), "nested/out.sarif", "{}\n").expect("write");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("nested/out.sarif")).expect("report"),
            "{}\n"
        );
        let err = write_report_file(dir.path(), "nested", "{}\n").expect_err("is a directory");
        assert!(err.reason.contains("destination is a directory"), "{err}");
    }

    #[test]
    fn write_report_file_keeps_stale_content_on_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("out.sarif"), "stale\n").expect("stale");
        std::fs::create_dir(dir.path().join("blocked")).expect("mkdir");
        let err = write_report_file(dir.path(), "blocked", "{}\n").expect_err("directory");
        assert!(err.reason.contains("destination is a directory"), "{err}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out.sarif")).expect("stale kept"),
            "stale\n"
        );
    }

    #[test]
    #[cfg(unix)]
    fn write_report_file_reports_unwritable_destinations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).expect("mkdir");
        let mut permissions = std::fs::metadata(&locked).expect("metadata").permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&locked, permissions).expect("readonly");
        let result = write_report_file(dir.path(), "locked/out.sarif", "{}\n");
        let mut permissions = std::fs::metadata(&locked).expect("metadata").permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&locked, permissions).expect("writable");
        let err = result.expect_err("unwritable");
        assert!(err.destination.contains("locked"), "{err}");
        assert!(!err.reason.is_empty(), "{err}");
        assert!(!locked.join("out.sarif").exists());
    }

    #[test]
    fn write_report_file_resolves_absolute_destinations_as_given() {
        let dir = tempfile::tempdir().expect("tempdir");
        let absolute = dir.path().join("abs.sarif").display().to_string();
        write_report_file(dir.path(), &absolute, "{}\n").expect("absolute write");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("abs.sarif")).expect("report"),
            "{}\n"
        );
    }

    #[test]
    fn collect_targets_names_the_failure_by_stage() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bep = dir.path().join("missing.json");
        let (code, message) = collect_targets(&bep, "dx_results", dir.path()).expect_err("absent");
        assert_eq!(code, CODE_UNREADABLE_BEP);
        assert!(message.contains("failed to read build events"), "{message}");

        std::fs::write(&bep, "").expect("bep");
        let (code, message) = collect_targets(&bep, "", dir.path()).expect_err("empty group");
        assert_eq!(code, CODE_INVALID_BEP);
        assert!(message.contains("invalid BEP config"), "{message}");

        std::fs::write(&bep, "{not json").expect("bep");
        let (code, message) = collect_targets(&bep, "dx_results", dir.path()).expect_err("bad");
        assert_eq!(code, CODE_INVALID_BEP);
        assert!(message.contains("invalid build events"), "{message}");
    }

    #[test]
    fn offline_summary_marks_only_offline_invocations() {
        assert_eq!(
            offline_summary("Running audit security for //...".to_owned(), true),
            "Running audit security for //... (offline, cache-only)"
        );
        assert_eq!(
            offline_summary("Running audit security for //...".to_owned(), false),
            "Running audit security for //..."
        );
    }

    #[test]
    fn frozen_summary_marks_only_frozen_invocations() {
        assert_eq!(
            frozen_summary("Running update for //...".to_owned(), true),
            "Running update for //... (frozen, no resolution changes)"
        );
        assert_eq!(
            frozen_summary("Running update for //...".to_owned(), false),
            "Running update for //..."
        );
        assert_eq!(
            frozen_summary(
                offline_summary("Running update for //...".to_owned(), true),
                true
            ),
            "Running update for //... (offline, cache-only) (frozen, no resolution changes)"
        );
    }

    #[test]
    fn startup_outcome_emits_error_then_finished_with_incomplete_results() {
        let mut out = Vec::new();
        emit_startup_outcome(&mut out, "invalid_arguments", "boom", 2).expect("events");
        let text = String::from_utf8(out).expect("utf8");
        let mut lines = text.lines();
        let first: serde_json::Value =
            serde_json::from_str(lines.next().expect("error")).expect("json");
        assert_eq!(first["event"], "error");
        assert_eq!(first["code"], "invalid_arguments");
        assert_eq!(first["message"], "boom");
        let second: serde_json::Value =
            serde_json::from_str(lines.next().expect("finished")).expect("json");
        assert_eq!(second["event"], "command_finished");
        assert_eq!(second["exit_code"], 2);
        assert_eq!(second["results_complete"], false);
        assert!(lines.next().is_none(), "nothing follows the terminal event");
    }

    #[test]
    fn startup_outcome_still_finishes_when_the_code_is_empty() {
        let mut out = Vec::new();
        emit_startup_outcome(&mut out, "", "boom", 2).expect("finished anyway");
        let event: serde_json::Value = serde_json::from_slice(&out).expect("json");
        assert_eq!(event["event"], "command_finished");
    }

    #[test]
    fn startup_outcome_reports_a_broken_stdout() {
        assert_eq!(
            emit_startup_outcome(&mut BrokenPipeWriter, "invalid_arguments", "boom", 2),
            Err(128 + 13)
        );
        assert_eq!(
            emit_startup_outcome(&mut FailingWriter, "invalid_arguments", "boom", 2),
            Err(1)
        );
    }

    #[test]
    fn operational_returns_broken_pipe_on_truncated_stdout() {
        use crate::args::parse;
        let invocation = parse(&["status".to_owned(), "--output=json".to_owned()]).expect("parse");
        let mut err = Vec::new();
        let code = operational(
            &invocation,
            &mut BrokenPipeWriter,
            &mut err,
            "launch_failed",
            "boom",
        );
        assert_eq!(code, 128 + 13);
    }
}
