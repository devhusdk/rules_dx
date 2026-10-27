use dx_apply::{FileSystem, RealFileSystem};
use dx_bep::ArtifactReader;
use dx_digest::blake3 as digest;
use dx_output::{
    command_started, notice_event, write_event, ActionCounts, ChangeEvent, ChangeKind,
    FindingRecord, NoticeEvent, OutputMode, Span,
};
use dx_plan::args::{Command, Invocation};
use dx_plan::reports::Destination;
use dx_plan::resolve::QueryRunner;
use dx_process::{broken_pipe_code, operational_code, pre_exec_code, stdout_io_code, Runner};
use std::cell::Cell;
use std::io::{self, BufReader, Write};
use std::path::Path;

pub(crate) use super::failure::{
    emit_failure_events, error_event_of, report_failure, Cause, Failure, CODE_ANALYSIS_FAILED,
    CODE_APPLICATION_FAILED, CODE_BAZEL_FAILED, CODE_DISK_FULL, CODE_INVALID_ARGUMENTS,
    CODE_QUERY_FAILED, CODE_QUERY_LAUNCH_FAILED, CODE_TESTS_FAILED,
};

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

/// Writes one built event, or ends the stream with `invalid_result` when it failed to build.
pub(crate) fn emit_built(
    out: &mut dyn Write,
    built: Result<serde_json::Value, dx_output::OutputError>,
) -> Result<(), i32> {
    match built {
        Ok(event) => emit_event(out, &event),
        Err(error) => {
            emit_failure_events(
                out,
                &Failure::new(
                    CODE_INVALID_RESULT,
                    &format!("dx built an invalid event: {error}"),
                    operational_code(),
                ),
            )?;
            Err(operational_code())
        }
    }
}

/// Drops every stdout write after the first failure, so no later record lands.
pub(crate) struct LatchedOut<'a> {
    inner: &'a mut dyn Write,
    failed: Option<(io::ErrorKind, String)>,
}

impl<'a> LatchedOut<'a> {
    pub(crate) fn new(inner: &'a mut dyn Write) -> Self {
        LatchedOut {
            inner,
            failed: None,
        }
    }

    /// The run's exit code, or the stdout failure code once a write failed.
    pub(crate) fn exit_code(&mut self, code: i32) -> i32 {
        let _ = self.flush();
        match &self.failed {
            Some((kind, message)) => stdout_io_code(&io::Error::new(*kind, message.clone())),
            None => code,
        }
    }

    fn latch<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        match &result {
            Err(error) if error.kind() != io::ErrorKind::Interrupted => {
                self.failed = Some((error.kind(), error.to_string()));
            }
            _ => {}
        }
        result
    }

    fn refused(&self) -> Option<io::Error> {
        self.failed
            .as_ref()
            .map(|(kind, message)| io::Error::new(*kind, message.clone()))
    }
}

impl Write for LatchedOut<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(error) = self.refused() {
            return Err(error);
        }
        let result = self.inner.write(buf);
        self.latch(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(error) = self.refused() {
            return Err(error);
        }
        let result = self.inner.flush();
        self.latch(result)
    }
}

pub(crate) fn kept_child(err: &mut dyn Write, child: i32, delivery: i32) -> i32 {
    if child == 0 {
        return delivery;
    }
    let _ = writeln!(
        err,
        "dx: could not write the final events to stdout; keeping exit {child}"
    );
    child
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
    emit_built(out, command_started(invocation.command.name(), "default"))
}

/// Writes the `error` event for a Bazel run that exited nonzero, keeping its native status.
pub(crate) fn emit_bazel_exit(
    out: &mut dyn Write,
    code: &str,
    message: &str,
    status: i32,
) -> Result<(), i32> {
    emit_built(out, bazel_exit_event(code, message, status, "execute"))
}

pub const REASON_STALE_SOURCE: &str = "stale_source";
pub const REASON_UNREADABLE_SOURCE: &str = "unreadable_source";
pub const REASON_INVALID_EDITS: &str = "invalid_edits";
pub const REASON_WRITE_FAILED: &str = "write_failed";
pub const REASON_INCOMPLETE_COLLECTION: &str = "incomplete_collection";
pub const REASON_CONFLICTING_DIGESTS: &str = "conflicting_digests";
pub const REASON_CONFLICTING_CANDIDATES: &str = "conflicting_candidates";

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

pub(crate) use super::notice_codes::*;

pub(crate) const CODE_LAUNCH_FAILED: &str = "launch_failed";
pub(crate) const CODE_BAZEL_SIGNALLED: &str = "bazel_signalled";
/// Fires before a Bazel run at `dx_worktree::disk::DISK_FULL_BYTES` (1 GiB). Worktree
/// reclaim uses the looser `dx_worktree::disk::is_low` rule instead, so the two are
/// distinct signals.
pub(crate) const CODE_DISK_LOW: &str = "disk_low";
pub(crate) const CODE_UNREADABLE_BEP: &str = "unreadable_bep";
pub(crate) const CODE_INVALID_BEP: &str = "invalid_bep";
pub(crate) const CODE_DIFF_FAILED: &str = "diff_failed";
pub(crate) const CODE_REPORT_FAILED: &str = "report_failed";
pub(crate) const CODE_COLLECTION_FAILED: &str = "collection_failed";
pub(crate) const CODE_VERIFICATION_FAILED: &str = "verification_failed";
pub(crate) const CODE_NOT_CONVERGED: &str = "not_converged";
pub(crate) const CODE_COVERAGE_BELOW_MINIMUM: &str = "coverage_below_minimum";
pub(crate) const CODE_NO_COVERAGE_COLLECTOR: &str = "no_coverage_collector";
pub(crate) const CODE_COVERAGE_SOURCES_INCOMPLETE: &str = "coverage_sources_incomplete";
pub(crate) const CODE_COVERAGE_SOURCES: &str = "coverage_sources";
pub(crate) const CODE_CHANGED_LINE_BELOW_MINIMUM: &str = "changed_line_coverage_below_minimum";
pub(crate) const CODE_CHANGED_LINE_INCOMPLETE: &str = "changed_line_coverage_incomplete";
pub(crate) const CODE_CHANGED_LINE_UNAVAILABLE: &str = "changed_line_coverage_unavailable";
pub(crate) const CODE_CLEAN_FAILED: &str = "clean_failed";
pub(crate) const CODE_CLEAN_PLANNED: &str = "clean_planned";
pub(crate) const CODE_CLEAN_PRUNED: &str = "clean_pruned";
pub(crate) const CODE_CLEAN_PRESERVED_LEASED: &str = "clean_preserved_leased";
pub(crate) const CODE_CLEAN_PRESERVED_UNOBSERVED: &str = "clean_preserved_unobserved";
pub(crate) const CODE_INVALID_RESULT: &str = "invalid_result";
pub(crate) const CODE_MANAGED_COMMIT_FAILED: &str = "managed_commit_failed";
pub(crate) const CODE_MANAGED_NO_CAPABILITY: &str = "no_capability";
pub(crate) const CODE_AUDIT_FAILED: &str = "audit_failed";
pub(crate) const CODE_UPDATE_FAILED: &str = "update_failed";
pub(crate) const CODE_BUMP_FAILED: &str = "bump_failed";
pub(crate) const CODE_ADD_FAILED: &str = "add_failed";
pub(crate) const CODE_MIGRATE_FAILED: &str = "migrate_failed";
pub(crate) const CODE_OFFLINE_REQUIRED: &str = "offline_required";
pub(crate) const CODE_FROZEN_LOCKED: &str = "frozen_locked";
#[cfg(test)]
pub(crate) const CODE_BACKEND_UNAVAILABLE: &str = "backend_unavailable";

/// How one umbrella phase ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhaseKind {
    Clean,
    Findings,
    NonConvergence,
    Selection,
    Tool,
    Incomplete,
    Report,
    /// Every failed Bazel action was a compile error.
    Compile,
    /// Bazel could not start, was signalled, or the disk is full.
    Host,
    Cancelled,
}

/// Bazel exit codes shared by every Bazel-backed phase.
pub(crate) const BAZEL_BUILD_FAILED: i32 = 1;
pub(crate) const BAZEL_USAGE: i32 = 2;
pub(crate) const BAZEL_TESTS_FAILED: i32 = 3;
pub(crate) const BAZEL_NO_TESTS: i32 = 4;
pub(crate) const BAZEL_INTERRUPTED: i32 = 8;

impl PhaseKind {
    /// Classifies a Bazel exit code; `findings` is the phase's own verdict that the code means findings.
    pub(crate) fn from_bazel_exit(code: i32, findings: bool) -> PhaseKind {
        match code {
            0 => PhaseKind::Clean,
            _ if findings => PhaseKind::Findings,
            BAZEL_BUILD_FAILED | BAZEL_TESTS_FAILED => PhaseKind::Incomplete,
            BAZEL_USAGE | BAZEL_NO_TESTS => PhaseKind::Selection,
            BAZEL_INTERRUPTED => PhaseKind::Cancelled,
            _ => PhaseKind::Tool,
        }
    }

    /// Reports whether later phases of `command` still run after a phase of this kind.
    pub(crate) fn continues(self, command: Command) -> bool {
        match self {
            PhaseKind::Selection | PhaseKind::Host | PhaseKind::Cancelled => false,
            PhaseKind::Compile => command != Command::Fix,
            _ => true,
        }
    }

    /// Names why the run stopped after a phase of this kind.
    pub(crate) fn stop_reason(self) -> &'static str {
        match self {
            PhaseKind::Clean | PhaseKind::Findings => "finished",
            PhaseKind::NonConvergence => "did not converge",
            PhaseKind::Selection => "rejected its scope or configuration",
            PhaseKind::Tool => "failed in Bazel",
            PhaseKind::Incomplete => "returned incomplete results",
            PhaseKind::Report => "failed to write its report",
            PhaseKind::Compile => "failed to compile",
            PhaseKind::Host => "failed to run its tools",
            PhaseKind::Cancelled => "was cancelled",
        }
    }
}

/// What one umbrella phase changed or refused in the workspace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PhaseEffects {
    pub(crate) changes: u64,
    pub(crate) applied: u64,
    pub(crate) not_applied: u64,
    pub(crate) baseline_refreshed: bool,
}

impl PhaseEffects {
    /// Reports whether the phase changed, refused or refreshed anything.
    pub(crate) fn touched(&self) -> bool {
        self.changes > 0 || self.not_applied > 0 || self.baseline_refreshed
    }
}

/// How one umbrella phase ended and what it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhaseRun {
    pub(crate) code: i32,
    pub(crate) kind: PhaseKind,
    pub(crate) effects: PhaseEffects,
}

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
    /// The pinned audit date, if any.
    pub audit_today: Option<&'a str>,
    /// Bazel action counts summed over the run's builds.
    pub actions: &'a ActionTally,
    /// The process table `dx clean` observes for live leases; `/proc` in production.
    pub proc_root: &'a Path,
}

/// Sums the Bazel action counts of every build in one run.
#[derive(Debug, Default)]
pub struct ActionTally(Cell<Option<ActionCounts>>);

impl ActionTally {
    pub fn record(&self, counts: ActionCounts) {
        let total = self.0.get().map_or(counts, |total| total + counts);
        self.0.set(Some(total));
    }

    pub fn total(&self) -> Option<ActionCounts> {
        self.0.get()
    }
}

/// Reads the action counts of one build event file and records them in `tally`.
pub(crate) fn read_actions(bep: &Path, tally: &ActionTally) -> Option<ActionCounts> {
    let file = std::fs::File::open(bep).ok()?;
    let summary = dx_bep::collect_action_summary(BufReader::new(file))?;
    let counts = ActionCounts {
        created: summary.created,
        executed: summary.executed,
        disk: summary.disk_cache_hits,
        remote: summary.remote_cache_hits,
        local: summary.local_cache_hits,
    };
    tally.record(counts);
    Some(counts)
}

/// Prints the run's action total on stderr in visible text mode.
pub fn print_actions(output: OutputMode, tally: &ActionTally, err: &mut dyn Write) {
    if !dx_output::dx_text_visible(&output) {
        return;
    }
    if let Some(line) = tally.total().map(|counts| actions_line(&counts)) {
        let _ = writeln!(err, "{line}");
    }
}

fn actions_line(counts: &ActionCounts) -> String {
    let cached = counts.cached();
    format!(
        "dx: {} actions, {} executed, {cached} cached",
        counts.executed.saturating_add(cached),
        counts.executed
    )
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
    let account = collect_output_account(bep, group, workspace)?;
    if let Some(problem) = account.problem {
        return Err((
            CODE_INVALID_BEP.to_owned(),
            format!("invalid build events: {problem}"),
        ));
    }
    Ok(account.targets)
}

/// Reads one build event file and returns one output group's outputs with the stream's gaps.
pub(crate) fn collect_output_account(
    bep: &Path,
    group: &str,
    workspace: &Path,
) -> Result<dx_bep::OutputAccount, (String, String)> {
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
    dx_bep::collect_account(BufReader::new(file), &config, &FsArtifacts, Some(workspace)).map_err(
        |err| {
            (
                CODE_INVALID_BEP.to_owned(),
                format!("invalid build events: {err}"),
            )
        },
    )
}

#[derive(Clone)]
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

fn place(path: Option<&String>, span: Option<Span>) -> String {
    match (path, span) {
        (Some(path), Some((start, _))) => format!("{path}:{}:{}", start.line, start.column),
        (Some(path), None) => path.clone(),
        (None, _) => "(no file)".to_owned(),
    }
}

pub(crate) fn text_finding(record: &FindingRecord) -> String {
    let diagnostic = &record.diagnostic;
    let span = diagnostic.range.and(record.span);
    let mut line = format!(
        "{}: {}: {} [{}",
        place(diagnostic.path.as_ref(), span),
        diagnostic.severity.name(),
        diagnostic.message,
        diagnostic.tool
    );
    if let Some(rule) = &diagnostic.rule {
        if rule.starts_with(&format!("{}/", diagnostic.tool)) {
            line.truncate(line.len() - diagnostic.tool.len());
        } else {
            line.push('/');
        }
        line.push_str(rule);
    }
    line.push(']');
    if diagnostic.fixable {
        line.push_str(" (fixable)");
    }
    if record.count > 1 {
        line.push_str(&format!(" (x{})", record.count));
    }
    for (index, item) in diagnostic.related.iter().enumerate() {
        line.push_str("\n  ");
        line.push_str(&item.role);
        line.push_str(": ");
        line.push_str(&item.message);
        if let Some(path) = &item.path {
            let span = item
                .range
                .and(record.related_spans.get(index).copied().flatten());
            line.push_str(" (");
            line.push_str(&place(Some(path), span));
            line.push(')');
        }
        if let Some(replacement) = &item.replacement {
            line.push_str(" suggests `");
            line.push_str(replacement);
            line.push('`');
            if let Some(applicability) = &item.applicability {
                line.push_str(" [");
                line.push_str(applicability);
                line.push(']');
            }
        }
    }
    line
}

/// Reports one failure found before the command executes.
pub(crate) fn pre_exec(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    message: &str,
) -> i32 {
    report_failure(
        output,
        out,
        err,
        &Failure::new(CODE_INVALID_ARGUMENTS, message, pre_exec_code()),
    )
}

/// Reports a refusal that is not an argument error, without usage text.
pub(crate) fn refusal(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    message: &str,
) -> i32 {
    report_failure(
        output,
        out,
        err,
        &Failure::new(CODE_INVALID_ARGUMENTS, message, pre_exec_code()).without_usage(),
    )
}

/// Reports a scope that did not resolve; a failed query never becomes an empty selection.
pub(crate) fn resolve_failed(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_plan::resolve::ResolveError,
) -> i32 {
    report_failure(output, out, err, &Failure::resolve(error))
}

/// The `error` event for a Bazel run or application that exited nonzero, keeping its status.
pub(crate) fn bazel_exit_event(
    code: &str,
    message: &str,
    status: i32,
    phase: &'static str,
) -> Result<serde_json::Value, dx_output::OutputError> {
    dx_output::failure_event(
        code,
        message,
        Some(phase),
        &dx_output::FailureFacts {
            cause: Cause::of(code).name(),
            status: Some(status),
            excerpt: None,
        },
    )
}

/// Reports one failed report as a `dx: report_failed:` line and, in JSON mode, an error event.
pub(crate) fn report_failed(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    detail: &str,
) -> Result<(), i32> {
    let _ = writeln!(err, "dx: {CODE_REPORT_FAILED}: {detail}");
    if let Some(next) = super::guidance::next_action(CODE_REPORT_FAILED) {
        let _ = writeln!(err, "{next}");
    }
    if output != OutputMode::Json {
        return Ok(());
    }
    let facts = dx_output::FailureFacts {
        cause: Cause::Report.name(),
        status: None,
        excerpt: None,
    };
    emit_built(
        out,
        dx_output::failure_event(CODE_REPORT_FAILED, detail, None, &facts),
    )
}

/// Writes one report document to its resolved destination and reports whether it landed.
pub(crate) fn write_report_file(
    workspace: &Path,
    destination: &str,
    document: &str,
) -> Result<(), ReportWriteError> {
    let target = dx_plan::reports::resolve_destination(workspace, destination);
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

pub(crate) fn operational_output(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    message: &str,
) -> i32 {
    report_failure(
        output,
        out,
        err,
        &Failure::new(code, message, operational_code()),
    )
}

pub(crate) fn operational(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    message: &str,
) -> i32 {
    operational_output(invocation.output, out, err, code, message)
}

/// Emits the `error` and `command_finished` pair for a failure before the command starts.
pub fn emit_startup_outcome(
    out: &mut dyn Write,
    code: &str,
    message: &str,
    exit: i32,
) -> Result<(), i32> {
    emit_failure_events(out, &Failure::new(code, message, exit))
}

/// Runs one Bazel argv and maps a spawn failure and a signalled Bazel to operational exits.
pub(crate) fn run_bazel_output(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    workspace: &Path,
    runner: &dyn Runner,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<i32, i32> {
    warn_low_disk(output, out, err, runner, workspace, argv)?;
    let status = runner.run(argv, workspace, env);
    bazel_exit(output, out, err, runner, workspace, argv, status)
}

/// Runs one Bazel argv that stops early once `withdraw` holds; `Ok(None)` means withdrawn.
pub(crate) fn run_bazel_withdrawable(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    workspace: &Path,
    runner: &dyn Runner,
    argv: &[String],
    withdraw: &dyn Fn() -> bool,
) -> Result<Option<i32>, i32> {
    warn_low_disk(invocation.output, out, err, runner, workspace, argv)?;
    match runner
        .run_withdrawable(argv, workspace, &[], withdraw)
        .transpose()
    {
        None => Ok(None),
        Some(status) => {
            bazel_exit(invocation.output, out, err, runner, workspace, argv, status).map(Some)
        }
    }
}

/// Warns on stderr, and with a JSON notice, when a disk Bazel writes to is nearly full.
fn warn_low_disk(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    runner: &dyn Runner,
    workspace: &Path,
    argv: &[String],
) -> Result<(), i32> {
    let Some((path, free)) =
        dx_plan::disk::shortage(&|p| runner.free_disk_bytes(p), workspace, argv)
    else {
        return Ok(());
    };
    let message = dx_plan::disk::low_message(&path, free);
    let _ = writeln!(err, "dx: {CODE_DISK_LOW}: {message}");
    if output != OutputMode::Json {
        return Ok(());
    }
    emit_built(
        out,
        notice_event(&NoticeEvent::warning(CODE_DISK_LOW, message)),
    )?;
    flush_out(out)
}

/// Reports a Bazel host failure, such as a full disk, that the exit code alone cannot name.
fn host_failure(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    runner: &dyn Runner,
    workspace: &Path,
    argv: &[String],
    code: i32,
) -> Option<i32> {
    if code != dx_plan::disk::BAZEL_LOCAL_ENVIRONMENT_EXIT {
        return None;
    }
    let (path, free) = dx_plan::disk::shortage(&|p| runner.free_disk_bytes(p), workspace, argv)?;
    let mut failure = Failure::new(
        CODE_DISK_FULL,
        &dx_plan::disk::full_message(code, &path, free),
        code,
    )
    .without_usage();
    failure.status = Some(code);
    failure.phase = Some("execute");
    Some(report_failure(output, out, err, &failure))
}

fn child_exit(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    status: io::Result<dx_process::ChildStatus>,
) -> Result<i32, i32> {
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            return Err(operational_output(
                output,
                out,
                err,
                CODE_LAUNCH_FAILED,
                &format!("failed to launch Bazel: {error}"),
            ));
        }
    };
    match status.code {
        Some(code) => Ok(code),
        None => Err(operational_output(
            output,
            out,
            err,
            CODE_BAZEL_SIGNALLED,
            "Bazel terminated by signal",
        )),
    }
}

fn bazel_exit(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    runner: &dyn Runner,
    workspace: &Path,
    argv: &[String],
    status: io::Result<dx_process::ChildStatus>,
) -> Result<i32, i32> {
    let code = child_exit(output, out, err, status)?;
    match host_failure(output, out, err, runner, workspace, argv, code) {
        Some(exit) => Err(exit),
        None => Ok(code),
    }
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
    run_bazel_output(invocation.output, out, err, workspace, runner, argv, env)
}

/// How one `bazel run` app went out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppExit {
    /// The app's exit code, or 128 plus the signal that ended it.
    pub(crate) code: i32,
    /// The signal that ended the app, when it died by one.
    pub(crate) signal: Option<i32>,
}

fn app_exit(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    status: io::Result<dx_process::ChildStatus>,
) -> Result<AppExit, i32> {
    if let Ok(dx_process::ChildStatus {
        code: None,
        signal: Some(signo),
    }) = status
    {
        return Ok(AppExit {
            code: dx_process::interrupted_code(signo),
            signal: Some(signo),
        });
    }
    child_exit(output, out, err, status).map(|code| AppExit { code, signal: None })
}

/// Writes the text line for an app that died by `signo`.
pub(crate) fn app_signalled_line(err: &mut dyn Write, label: &str, signo: i32) {
    let _ = writeln!(
        err,
        "dx: application_failed: bazel run {label} terminated by signal {signo}"
    );
}

/// Launches one `bazel run` app; its exit is the app's, and a signalled app exits 128 plus the signal.
pub(crate) fn run_bazel_app(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    workspace: &Path,
    runner: &dyn Runner,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<AppExit, i32> {
    let status = runner.run(argv, workspace, env);
    app_exit(invocation.output, out, err, status)
}

/// Launches one `bazel run` app that stops early once `withdraw` holds; `Ok(None)` means withdrawn.
pub(crate) fn run_bazel_app_withdrawable(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    workspace: &Path,
    runner: &dyn Runner,
    argv: &[String],
    withdraw: &dyn Fn() -> bool,
) -> Result<Option<AppExit>, i32> {
    match runner
        .run_withdrawable(argv, workspace, &[], withdraw)
        .transpose()
    {
        None => Ok(None),
        Some(status) => app_exit(invocation.output, out, err, status).map(Some),
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
    #[test]
    fn from_bazel_exit_classifies_every_code() {
        let table = [
            (0, false, PhaseKind::Clean),
            (0, true, PhaseKind::Clean),
            (1, false, PhaseKind::Incomplete),
            (2, false, PhaseKind::Selection),
            (3, false, PhaseKind::Incomplete),
            (3, true, PhaseKind::Findings),
            (4, false, PhaseKind::Selection),
            (8, false, PhaseKind::Cancelled),
            (36, false, PhaseKind::Tool),
        ];
        for (code, findings, kind) in table {
            assert_eq!(PhaseKind::from_bazel_exit(code, findings), kind, "{code}");
        }
    }

    use super::*;

    #[test]
    fn actions_line_formats_action_counts() {
        let tally = ActionTally::default();
        let mut err = Vec::new();
        print_actions(OutputMode::Text { quiet: false }, &tally, &mut err);
        assert!(err.is_empty());
        let counts = ActionCounts {
            created: 50,
            executed: 3,
            disk: 1,
            remote: 2,
            local: 4,
        };
        tally.record(counts);
        tally.record(counts);
        print_actions(OutputMode::Json, &tally, &mut err);
        print_actions(OutputMode::Text { quiet: true }, &tally, &mut err);
        assert!(err.is_empty());
        print_actions(OutputMode::Text { quiet: false }, &tally, &mut err);
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "dx: 20 actions, 6 executed, 14 cached\n"
        );
    }
    use dx_output::{command_finished, DiagnosticEvent, FinishedCounts, Severity};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    struct Flaky {
        written: Vec<u8>,
        calls: usize,
        fail_on: usize,
        kind: io::ErrorKind,
    }

    impl Write for Flaky {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls == self.fail_on {
                return Err(io::Error::new(self.kind, "stdout failed"));
            }
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn phase_effects_touched_counts_changes_refusals_and_baselines() {
        assert!(!PhaseEffects::default().touched());
        let applied_only = PhaseEffects {
            applied: 1,
            ..PhaseEffects::default()
        };
        assert!(!applied_only.touched());
        for effects in [
            PhaseEffects {
                changes: 1,
                applied: 1,
                ..PhaseEffects::default()
            },
            PhaseEffects {
                not_applied: 1,
                ..PhaseEffects::default()
            },
            PhaseEffects {
                baseline_refreshed: true,
                ..PhaseEffects::default()
            },
        ] {
            assert!(effects.touched(), "{effects:?}");
        }
    }

    fn flaky(fail_on: usize, kind: io::ErrorKind) -> Flaky {
        Flaky {
            written: Vec::new(),
            calls: 0,
            fail_on,
            kind,
        }
    }

    #[test]
    fn latched_out_refuses_every_write_after_a_broken_pipe() {
        let mut inner = flaky(2, io::ErrorKind::BrokenPipe);
        let mut latched = LatchedOut::new(&mut inner);
        latched.write_all(b"first\n").expect("first write lands");
        assert!(latched.write_all(b"second\n").is_err());
        assert!(latched.write_all(b"third\n").is_err());
        assert!(latched.flush().is_err());
        assert_eq!(latched.exit_code(0), 141);
        assert_eq!(inner.written, b"first\n");
        assert_eq!(inner.calls, 2, "nothing reaches stdout after the failure");
    }

    #[test]
    fn latched_out_maps_other_failures_to_operational() {
        let mut inner = flaky(1, io::ErrorKind::Other);
        let mut latched = LatchedOut::new(&mut inner);
        assert!(latched.write_all(b"first\n").is_err());
        assert_eq!(latched.exit_code(0), 1);
    }

    #[test]
    fn latched_out_retries_interrupted_writes_and_keeps_the_code() {
        let mut inner = flaky(1, io::ErrorKind::Interrupted);
        let mut latched = LatchedOut::new(&mut inner);
        latched
            .write_all(b"first\n")
            .expect("interrupted writes retry");
        assert_eq!(latched.exit_code(3), 3);
        assert_eq!(inner.written, b"first\n");
    }

    struct ProbeRunner {
        code: Option<i32>,
        signal: Option<i32>,
        spawn_error: bool,
        seen: Rc<RefCell<(Vec<String>, Vec<(String, String)>)>>,
        free: Option<u64>,
        probed: RefCell<Vec<PathBuf>>,
    }

    impl Runner for ProbeRunner {
        fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
            self.probed.borrow_mut().push(path.to_path_buf());
            self.free
        }

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
            Ok(dx_process::ChildStatus {
                code: self.code,
                signal: self.signal,
            })
        }
    }

    fn bazel_invocation() -> Invocation {
        dx_plan::args::parse(&["build".to_owned(), "//...".to_owned()]).expect("parse")
    }

    #[test]
    fn run_bazel_forwards_argv_and_env_and_returns_the_exit_code() {
        let seen = Rc::new(RefCell::new((Vec::new(), Vec::new())));
        let runner = ProbeRunner {
            code: Some(7),
            signal: None,
            spawn_error: false,
            seen: Rc::clone(&seen),
            free: None,
            probed: RefCell::new(Vec::new()),
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
            signal: None,
            spawn_error: true,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
            free: None,
            probed: RefCell::new(Vec::new()),
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
            signal: None,
            spawn_error: false,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
            free: None,
            probed: RefCell::new(Vec::new()),
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

    fn disk_runner(code: i32, free: Option<u64>) -> ProbeRunner {
        ProbeRunner {
            code: Some(code),
            signal: None,
            spawn_error: false,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
            free,
            probed: RefCell::new(Vec::new()),
        }
    }

    fn run_on_disk(runner: &ProbeRunner, output: OutputMode) -> (Result<i32, i32>, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let argv = vec![
            "bazel".to_owned(),
            "--output_base=/ob".to_owned(),
            "build".to_owned(),
        ];
        let code = run_bazel_output(
            output,
            &mut out,
            &mut err,
            Path::new("/ws"),
            runner,
            &argv,
            &[],
        );
        (
            code,
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    #[test]
    fn run_bazel_app_keeps_exit_36_on_a_nearly_full_disk() {
        for output in [OutputMode::Text { quiet: false }, OutputMode::Json] {
            let runner = disk_runner(36, Some(1 << 20));
            let mut invocation = bazel_invocation();
            invocation.output = output;
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = run_bazel_app(
                &invocation,
                &mut out,
                &mut err,
                Path::new("/ws"),
                &runner,
                &["bazel".to_owned(), "run".to_owned(), "//app".to_owned()],
                &[],
            );
            assert_eq!(
                code,
                Ok(AppExit {
                    code: 36,
                    signal: None
                })
            );
            assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
            let err = String::from_utf8(err).expect("utf8");
            assert!(!err.contains("disk_"), "{err}");
            assert!(runner.probed.borrow().is_empty());
        }
    }

    #[test]
    fn run_bazel_app_still_reports_spawn_failures_and_signals() {
        for (spawn_error, code, expected) in [
            (true, Some(0), "launch_failed"),
            (false, None, "bazel_signalled"),
        ] {
            let runner = ProbeRunner {
                code,
                signal: None,
                spawn_error,
                seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
                free: Some(1 << 20),
                probed: RefCell::new(Vec::new()),
            };
            let invocation = bazel_invocation();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let exit = run_bazel_app(
                &invocation,
                &mut out,
                &mut err,
                Path::new("/ws"),
                &runner,
                &["bazel".to_owned()],
                &[],
            );
            assert_eq!(exit, Err(operational_code()));
            let err = String::from_utf8(err).expect("utf8");
            assert!(err.starts_with(&format!("dx: {expected}: ")), "{err}");
            assert!(runner.probed.borrow().is_empty());
        }
    }

    fn signal_runner(signal: Option<i32>) -> ProbeRunner {
        ProbeRunner {
            code: None,
            signal,
            spawn_error: false,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
            free: None,
            probed: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn run_bazel_app_maps_a_signalled_app_to_128_plus_the_signal() {
        for output in [OutputMode::Text { quiet: false }, OutputMode::Json] {
            let runner = signal_runner(Some(11));
            let mut invocation = bazel_invocation();
            invocation.output = output;
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let exit = run_bazel_app(
                &invocation,
                &mut out,
                &mut err,
                Path::new("/ws"),
                &runner,
                &["bazel".to_owned(), "run".to_owned(), "//app".to_owned()],
                &[],
            );
            assert_eq!(
                exit,
                Ok(AppExit {
                    code: 139,
                    signal: Some(11)
                })
            );
            assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
            assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
        }
    }

    #[test]
    fn run_bazel_keeps_bazel_signalled_when_the_signal_is_known() {
        let runner = signal_runner(Some(11));
        let invocation = bazel_invocation();
        let (mut out, mut err) = (Vec::new(), Vec::new());
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
    fn disk_full_names_exit_36_on_a_nearly_full_disk() {
        let runner = disk_runner(36, Some(1 << 20));
        let (code, _, err) = run_on_disk(&runner, OutputMode::Text { quiet: false });
        assert_eq!(code, Err(36));
        assert!(
            err.contains("dx: disk_low: 1.0 MiB free at /ws; free disk space with `dx worktree prune --apply`\n"),
            "{err}"
        );
        assert!(
            err.contains(
                "dx: disk_full: Bazel exited 36 with 1.0 MiB free at /ws; the disk is full\n"
            ),
            "{err}"
        );
        assert!(
            err.contains("next: free disk space (try `dx worktree prune --apply`) and run the command again.\n"),
            "{err}"
        );
        assert!(runner.probed.borrow().contains(&PathBuf::from("/ob")));

        let runner = disk_runner(36, Some(1 << 20));
        let (code, out, _) = run_on_disk(&runner, OutputMode::Json);
        assert_eq!(code, Err(36));
        let events: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).expect("NDJSON"))
            .collect();
        let notices: Vec<_> = events.iter().filter(|e| e["event"] == "notice").collect();
        assert_eq!(notices.len(), 1, "{out}");
        assert_eq!(notices[0]["code"], "disk_low");
        assert_eq!(notices[0]["level"], "warning");
        let errors: Vec<_> = events.iter().filter(|e| e["event"] == "error").collect();
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(errors[0]["code"], "disk_full");
        assert_eq!(errors[0]["cause"], "host");
        assert_eq!(errors[0]["status"], 36);
        assert_eq!(errors[0]["phase"], "execute");
        let finished: Vec<_> = events
            .iter()
            .filter(|e| e["event"] == "command_finished")
            .collect();
        assert_eq!(finished.len(), 1, "{out}");
        assert_eq!(finished[0]["exit_code"], 36);
    }

    #[test]
    fn exit_36_with_room_on_disk_keeps_the_bazel_exit() {
        let runner = disk_runner(36, Some(100 << 30));
        let (code, out, err) = run_on_disk(&runner, OutputMode::Json);
        assert_eq!(code, Ok(36));
        assert_eq!(out, "");
        assert_eq!(err, "");
    }

    #[test]
    fn other_exits_on_a_nearly_full_disk_only_warn() {
        let runner = disk_runner(1, Some(1 << 20));
        let (code, _, err) = run_on_disk(&runner, OutputMode::Text { quiet: false });
        assert_eq!(code, Ok(1));
        assert!(err.starts_with("dx: disk_low: "), "{err}");
        assert!(!err.contains("disk_full"), "{err}");
        assert_eq!(
            runner.probed.borrow().len(),
            dx_plan::disk::disk_paths(Path::new("/ws"), &["--output_base=/ob".to_owned()]).len()
        );
    }

    #[test]
    fn exit_36_without_a_disk_reading_keeps_the_bazel_exit() {
        let runner = disk_runner(36, None);
        let (code, out, err) = run_on_disk(&runner, OutputMode::Json);
        assert_eq!(code, Ok(36));
        assert_eq!(out, "");
        assert_eq!(err, "");
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

    fn finding(path: Option<&str>, range: Option<(u64, u64)>) -> FindingRecord {
        FindingRecord::single(DiagnosticEvent {
            rule: Some("rule".to_owned()),
            path: path.map(str::to_owned),
            range,
            fixable: true,
            ..DiagnosticEvent::new(Severity::Warning, "lint-tool", "mapped")
        })
    }

    fn at(line: u64, column: u64) -> dx_output::Position {
        dx_output::Position { line, column }
    }

    #[test]
    fn text_finding_prints_the_verified_location_tool_rule_severity_and_message() {
        let mut record = finding(Some("src/a.py"), Some((4, 6)));
        record.span = Some((at(2, 3), at(2, 5)));
        assert_eq!(
            text_finding(&record),
            "src/a.py:2:3: warning: mapped [lint-tool/rule] (fixable)"
        );
        record.count = 3;
        record.diagnostic.fixable = false;
        assert_eq!(
            text_finding(&record),
            "src/a.py:2:3: warning: mapped [lint-tool/rule] (x3)"
        );
    }

    #[test]
    fn text_finding_does_not_repeat_a_tool_prefix_in_the_rule() {
        let mut record = finding(Some("src/a.py"), None);
        record.diagnostic.tool = "vuln".to_owned();
        record.diagnostic.rule = Some("vuln/GHSA-1".to_owned());
        assert!(text_finding(&record).contains("[vuln/GHSA-1]"));
    }

    #[test]
    fn text_finding_never_guesses_a_location() {
        let unverified = finding(Some("src/a.py"), Some((4, 6)));
        assert_eq!(
            text_finding(&unverified),
            "src/a.py: warning: mapped [lint-tool/rule] (fixable)"
        );
        let mut pathless = finding(None, None);
        pathless.diagnostic.rule = None;
        pathless.diagnostic.fixable = false;
        assert_eq!(
            text_finding(&pathless),
            "(no file): warning: mapped [lint-tool]"
        );
        let mut stray = finding(Some("src/a.py"), None);
        stray.span = Some((at(9, 9), at(9, 9)));
        assert!(text_finding(&stray).starts_with("src/a.py: warning"));
    }

    #[test]
    fn text_finding_places_related_items_and_suggestions() {
        let mut record = finding(None, None);
        record.diagnostic.fixable = false;
        record.diagnostic.related = vec![
            dx_output::RelatedEvent {
                role: "label".to_owned(),
                message: "expected `i32`".to_owned(),
                path: Some("src/b.rs".to_owned()),
                range: Some((1, 2)),
                replacement: None,
                applicability: None,
            },
            dx_output::RelatedEvent {
                role: "help".to_owned(),
                message: "try this".to_owned(),
                path: None,
                range: None,
                replacement: Some("1".to_owned()),
                applicability: Some("MaybeIncorrect".to_owned()),
            },
        ];
        record.related_spans = vec![Some((at(7, 2), at(7, 3))), None];
        assert_eq!(
            text_finding(&record),
            "(no file): warning: mapped [lint-tool/rule]\n  label: expected `i32` (src/b.rs:7:2)\n  help: try this suggests `1` [MaybeIncorrect]"
        );
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
        let broken_out = dx_output::OutputError::from(io::Error::from(io::ErrorKind::BrokenPipe));
        assert!(broken_out.is_broken_pipe());
        assert_eq!(stdout_output_code(&broken_out), 128 + 13);
        let other_out = dx_output::OutputError::from(io::Error::other("boom"));
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
        assert_eq!(
            report_failed(
                &mut out,
                &mut err,
                OutputMode::Text { quiet: false },
                "boom",
            ),
            Ok(())
        );
        assert_eq!(String::from_utf8(err).unwrap(), "dx: report_failed: boom\nnext: make the report destination writable and run the command again.\n");
        assert!(out.is_empty());

        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            report_failed(&mut out, &mut err, OutputMode::Diff, "boom"),
            Ok(())
        );
        assert_eq!(String::from_utf8(err).unwrap(), "dx: report_failed: boom\nnext: make the report destination writable and run the command again.\n");
        assert!(out.is_empty());

        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            report_failed(&mut out, &mut err, OutputMode::Json, "boom"),
            Ok(())
        );
        assert_eq!(String::from_utf8(err).unwrap(), "dx: report_failed: boom\nnext: make the report destination writable and run the command again.\n");
        let event: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(event["code"], CODE_REPORT_FAILED);
        assert_eq!(event["message"], "boom");
    }

    #[test]
    fn report_failed_returns_the_stdout_code_when_its_event_cannot_land() {
        let mut err = Vec::new();
        assert_eq!(
            report_failed(&mut BrokenPipeWriter, &mut err, OutputMode::Json, "boom"),
            Err(128 + 13)
        );
    }

    #[test]
    fn emit_built_writes_a_built_event_like_emit_event() {
        let event = command_finished(0, &FinishedCounts::default());
        let mut out = Vec::new();
        assert_eq!(emit_built(&mut out, Ok(event.clone())), Ok(()));
        let written: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(written, event);
        assert_eq!(
            emit_built(&mut BrokenPipeWriter, Ok(event.clone())),
            emit_event(&mut BrokenPipeWriter, &event)
        );
        assert_eq!(emit_built(&mut BrokenPipeWriter, Ok(event)), Err(128 + 13));
    }

    #[test]
    fn emit_built_returns_the_write_code_when_the_invalid_result_cannot_land() {
        assert_eq!(
            emit_built(&mut BrokenPipeWriter, command_started("", "default")),
            Err(128 + 13)
        );
        assert_eq!(
            emit_built(&mut FailingWriter, command_started("", "default")),
            Err(operational_code())
        );
    }

    #[test]
    fn emit_built_ends_the_stream_when_an_event_fails_to_build() {
        let mut out = Vec::new();
        let built = command_started("", "default");
        assert!(built.is_err());
        assert_eq!(emit_built(&mut out, built), Err(operational_code()));
        let lines: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0]["event"], "error");
        assert_eq!(lines[0]["code"], CODE_INVALID_RESULT);
        assert_eq!(lines[1]["event"], "command_finished");
        assert_eq!(lines[1]["results_complete"], false);
        assert_eq!(lines[1]["exit_code"], operational_code());
    }

    #[test]
    fn a_later_flush_failure_keeps_the_earlier_write_failure() {
        struct WriteThenFlush;
        impl Write for WriteThenFlush {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }
        let mut inner = WriteThenFlush;
        let mut latched = LatchedOut::new(&mut inner);
        assert!(latched.write_all(b"x").is_err());
        assert_eq!(latched.exit_code(0), operational_code());
    }

    #[test]
    fn a_flush_failure_alone_fails_a_clean_run() {
        let mut inner = BrokenPipeWriter;
        let mut latched = LatchedOut::new(&mut inner);
        assert_eq!(latched.exit_code(0), 128 + 13);
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
    fn collect_targets_rejects_a_cut_off_last_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bep = dir.path().join("cut.json");
        std::fs::write(
            &bep,
            "{\"id\": {\"namedSetOfFiles\": {\"id\": \"1\"}}}\n{\"id\":{\"targetCo",
        )
        .expect("bep");
        let (code, message) = collect_targets(&bep, "dx_results", dir.path()).expect_err("cut");
        assert_eq!(code, CODE_INVALID_BEP);
        assert!(
            message.starts_with("invalid build events: line 2: invalid JSON"),
            "{message}"
        );
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
        let lines: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0]["event"], "error");
        assert_eq!(lines[0]["code"], CODE_INVALID_RESULT);
        assert_eq!(lines[1]["event"], "command_finished");
        assert_eq!(lines[1]["exit_code"], 2);
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
        use dx_plan::args::parse;
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
