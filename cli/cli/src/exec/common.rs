use dx_apply::{FileSystem, RealFileSystem};
use dx_bep::ArtifactReader;
use dx_digest::blake3 as digest;
use dx_output::{
    ActionCounts, ChangeEvent, ChangeKind, DiagnosticEvent, FindingRecord, NoticeEvent, OutputMode,
    Severity, Span, command_started, notice_event, write_event,
};
use dx_plan::args::Invocation;
use dx_plan::reports::Destination;
use dx_plan::resolve::QueryRunner;
use dx_process::{Runner, broken_pipe_code, operational_code, pre_exec_code, stdout_io_code};
use std::cell::Cell;
use std::io::{self, BufReader, Write};
use std::path::Path;

pub(crate) use super::failure::{
    CODE_ANALYSIS_FAILED, CODE_APPLICATION_FAILED, CODE_BAZEL_FAILED, CODE_DISK_FULL,
    CODE_INVALID_ARGUMENTS, CODE_SERVER_CRASHED, CODE_SERVER_OUT_OF_MEMORY, CODE_TESTS_FAILED,
    Cause, Failure, emit_failure_events, report_failure,
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
pub(crate) fn bazel_failure_code(bazel_code: i32) -> &'static str {
    if bazel_code == BAZEL_OUT_OF_MEMORY {
        CODE_SERVER_OUT_OF_MEMORY
    } else if bazel_code == crate::bazel_restart::RESTART_EXIT {
        CODE_SERVER_CRASHED
    } else {
        super::bazel_hint::refine(CODE_BAZEL_FAILED)
    }
}

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
pub(crate) const CODE_SCOPE_EXCLUDED: &str = "scope_excluded";
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
pub(crate) const CODE_CLEAN_REMOVED: &str = "clean_removed";
pub(crate) const CODE_INVALID_RESULT: &str = "invalid_result";
pub(crate) const CODE_GENERATION_FAILED: &str = "generation_failed";
pub(crate) const CODE_MANAGED_COMMIT_FAILED: &str = "managed_commit_failed";
pub(crate) const CODE_MANAGED_NO_CAPABILITY: &str = "no_capability";
pub(crate) const CODE_AUDIT_FAILED: &str = "audit_failed";
pub(crate) const CODE_UPDATE_FAILED: &str = "update_failed";
pub(crate) const CODE_UPDATE_WIDEN_FAILED: &str = "update_widen_failed";
pub(crate) const CODE_OFFLINE_REQUIRED: &str = "offline_required";
pub(crate) const CODE_FROZEN_LOCKED: &str = "frozen_locked";
pub(crate) const CODE_DEPLOY_NEEDS_HUMAN: &str = "deploy_needs_human";
/// Runs `git` in `workspace` and returns its stdout; a launch error or a non-zero exit is an error.
pub(super) fn git_output(
    runner: &dyn QueryRunner,
    workspace: &Path,
    what: &str,
    args: &[&str],
) -> Result<Vec<u8>, dx_error::DxError> {
    let mut argv = vec!["git".to_owned()];
    argv.extend(args.iter().map(ToString::to_string));
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| dx_error::DxError::infra(format!("git {what} failed: {error}")))?;
    if result.code != Some(0) {
        let detail = dx_output::first_diagnostic_line(&result.stderr, 200, "no Git diagnostic");
        return Err(dx_error::DxError::failure(format!(
            "git {what} failed: {detail}"
        )));
    }
    Ok(result.stdout)
}

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
    /// Generation failed, so later phases would run on stale files.
    GenerateFailed,
}

/// Bazel exit codes shared by every Bazel-backed phase.
pub(crate) const BAZEL_BUILD_FAILED: i32 = 1;
pub(crate) const BAZEL_USAGE: i32 = 2;
pub(crate) const BAZEL_TESTS_FAILED: i32 = 3;
pub(crate) const BAZEL_NO_TESTS: i32 = 4;
pub(crate) const BAZEL_INTERRUPTED: i32 = 8;
pub(crate) const BAZEL_OUT_OF_MEMORY: i32 = 33;

/// Reports whether Bazel's server ran out of memory or died; a fresh server will not help later phases.
pub(crate) fn server_died(code: i32) -> bool {
    code == BAZEL_OUT_OF_MEMORY || code == crate::bazel_restart::RESTART_EXIT
}

impl PhaseKind {
    /// Classifies a Bazel exit code; `findings` is the phase's own verdict that the code means findings.
    pub(crate) fn from_bazel_exit(code: i32, findings: bool) -> PhaseKind {
        match code {
            0 => PhaseKind::Clean,
            _ if server_died(code) => PhaseKind::Host,
            _ if findings => PhaseKind::Findings,
            BAZEL_BUILD_FAILED | BAZEL_TESTS_FAILED => PhaseKind::Incomplete,
            BAZEL_USAGE | BAZEL_NO_TESTS => PhaseKind::Selection,
            BAZEL_INTERRUPTED => PhaseKind::Cancelled,
            _ => PhaseKind::Tool,
        }
    }

    /// Reports whether later phases still run after a phase of this kind.
    pub(crate) fn continues(self, apply: bool) -> bool {
        match self {
            PhaseKind::Selection
            | PhaseKind::Host
            | PhaseKind::Cancelled
            | PhaseKind::GenerateFailed => false,
            PhaseKind::Compile => !apply,
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
            PhaseKind::GenerateFailed => "failed",
        }
    }
}

/// What one umbrella phase changed or refused in the workspace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PhaseEffects {
    pub(crate) changes: u64,
    pub(crate) applied: u64,
    pub(crate) not_applied: u64,
}

impl PhaseEffects {
    /// Reports whether the phase changed or refused anything.
    pub(crate) fn touched(&self) -> bool {
        self.changes > 0 || self.not_applied > 0
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
    /// The process table `dx sync` observes for live leases; `/proc` in production.
    pub proc_root: &'a Path,
    pub edit_commit_json: Option<&'a str>,
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

/// Prints on stderr in visible text mode that `retries` calls ran again without the remote cache.
pub fn print_cache_retries(output: OutputMode, retries: u32, err: &mut dyn Write) {
    if retries > 0 && dx_output::dx_text_visible(&output) {
        let _ = writeln!(
            err,
            "dx: {retries} Bazel call(s) ran again without the remote cache after a remote cache error"
        );
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

const UNREADABLE_BEP_PREFIX: &str = "failed to read build events: ";

/// The stable code for a [`collect_targets`] error.
pub(crate) fn bep_error_code(error: &dx_error::DxError) -> &'static str {
    if error.message().starts_with(UNREADABLE_BEP_PREFIX) {
        CODE_UNREADABLE_BEP
    } else {
        CODE_INVALID_BEP
    }
}

/// Reads one build event file and returns the target outputs of one output group.
pub(crate) fn collect_targets(
    bep: &Path,
    group: &str,
    workspace: &Path,
) -> Result<Vec<dx_bep::TargetOutput>, dx_error::DxError> {
    let account = collect_output_account(bep, group, workspace)
        .map_err(|(_, message)| dx_error::DxError::failure(message))?;
    if let Some(problem) = account.problem {
        return Err(dx_error::DxError::failure(format!(
            "invalid build events: {problem}"
        )));
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
            format!("{UNREADABLE_BEP_PREFIX}{err}"),
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

fn severity_word(severity: Severity, color: bool) -> String {
    let name = severity.name();
    let code = match severity {
        Severity::Error => "31",
        Severity::Warning => "33",
        Severity::Info => return name.to_owned(),
    };
    if color {
        format!("\x1b[{code}m{name}\x1b[0m")
    } else {
        name.to_owned()
    }
}

/// The closing stderr line of a text-mode lint, typecheck or format run.
pub(crate) fn findings_summary(diagnostics: &[DiagnosticEvent], targets: usize) -> String {
    let count = |severity: Severity| {
        diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    };
    let (errors, warnings, infos) = (
        count(Severity::Error),
        count(Severity::Warning),
        count(Severity::Info),
    );
    if diagnostics.is_empty() {
        return format!("no findings in {targets} target(s)");
    }
    let files: std::collections::BTreeSet<_> = diagnostics
        .iter()
        .filter_map(|d| d.path.as_deref())
        .collect();
    let fixable = diagnostics.iter().filter(|d| d.fixable).count();
    let mut line = format!("{errors} error(s), {warnings} warning(s)");
    if infos > 0 {
        line.push_str(&format!(", {infos} info"));
    }
    line.push_str(&format!(" in {} file(s)", files.len()));
    if fixable > 0 {
        line.push_str(&format!(", {fixable} fixable with --apply"));
    }
    line
}

pub(crate) fn text_finding(record: &FindingRecord) -> String {
    text_finding_colored(record, dx_output::color_enabled())
}

fn text_finding_colored(record: &FindingRecord, color: bool) -> String {
    let diagnostic = &record.diagnostic;
    let span = diagnostic.range.and(record.span);
    let mut line = format!(
        "{}: {}: {} [{}",
        place(diagnostic.path.as_ref(), span),
        severity_word(diagnostic.severity, color),
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
    } else if let Some(reason) = &diagnostic.unfixable_reason {
        line.push_str(&format!(" (not fixable: {reason})"));
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

/// Reports a [`dx_error::DxError`] refusal without usage text, exiting with the code its kind maps to.
pub(crate) fn refusal_dx_error(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_error::DxError,
) -> i32 {
    report_failure(
        output,
        out,
        err,
        &Failure::new(CODE_INVALID_ARGUMENTS, error.message(), error.exit_code()).without_usage(),
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
            next: None,
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
    report_failed_with_next(out, err, output, detail, None)
}

/// Like `report_failed`, with a next step that fits the cause instead of the default.
pub(crate) fn report_failed_with_next(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    detail: &str,
    next: Option<&str>,
) -> Result<(), i32> {
    let _ = writeln!(err, "dx: {CODE_REPORT_FAILED}: {detail}");
    let default = super::guidance::next_action(CODE_REPORT_FAILED);
    if let Some(next) = next.or(default) {
        let _ = writeln!(err, "{next}");
    }
    if output != OutputMode::Json {
        return Ok(());
    }
    let facts = dx_output::FailureFacts {
        cause: Cause::Report.name(),
        status: None,
        excerpt: None,
        next: None,
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

/// Reports a [`dx_error::DxError`] under `code`, exiting with the code its kind maps to.
pub(crate) fn report_dx_error(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    error: &dx_error::DxError,
) -> i32 {
    report_failure(
        output,
        out,
        err,
        &Failure::new(code, error.message(), error.exit_code()),
    )
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

/// Notes on stderr, and with a JSON notice, each named target a `dx.toml` exclude covers.
/// Named targets still run: the command line beats the config.
pub(crate) fn warn_scope_excluded(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    kept: &[String],
) -> Result<(), i32> {
    for label in kept {
        let message = format!(
            "{label} is excluded by dx.toml [scope] exclude; running it because it was named"
        );
        let _ = writeln!(err, "dx: {CODE_SCOPE_EXCLUDED}: {message}");
        if output == OutputMode::Json {
            emit_built(
                out,
                notice_event(&NoticeEvent::warning(CODE_SCOPE_EXCLUDED, message)),
            )?;
        }
    }
    Ok(())
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

/// Writes the text line for an app that exited with `code`.
pub(crate) fn app_exited_line(err: &mut dyn Write, label: &str, code: i32) {
    let _ = writeln!(err, "dx: {label} exited {code}");
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
    let status = runner
        .run_app(argv, workspace, env, None, &[])
        .map(|status| status.unwrap_or_else(|| dx_process::ChildStatus::signalled(None)));
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
        .run_app(argv, workspace, &[], Some(withdraw), &[])
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
#[path = "common_tests.rs"]
mod tests;
