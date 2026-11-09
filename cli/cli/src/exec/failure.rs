use super::common::{CODE_INVALID_RESULT, emit_event, flush_out, stdout_output_code};
use dx_output::{
    Excerpt, FailureFacts, FinishedCounts, OutputMode, command_finished, failure_event,
};
use dx_plan::resolve::{QueryFailure, ResolveError};
use dx_process::{interrupted, interrupted_code, operational_code, pre_exec_code};
use std::io::Write;

pub(crate) const CODE_INVALID_ARGUMENTS: &str = "invalid_arguments";
pub(crate) const CODE_INVALID_SCOPE: &str = "invalid_scope";
pub(crate) const CODE_NO_OWNER: &str = "no_owner";
pub(crate) const CODE_NO_TESTS: &str = "no_tests";
pub(crate) const CODE_NOT_DEPLOYABLE: &str = "not_deployable";
pub(crate) const CODE_QUERY_LAUNCH_FAILED: &str = "query_launch_failed";
pub(crate) const CODE_QUERY_FAILED: &str = "query_failed";
pub(crate) const CODE_BAZEL_FAILED: &str = "bazel_failed";
pub(crate) const CODE_TESTS_FAILED: &str = "tests_failed";
pub(crate) const CODE_APPLICATION_FAILED: &str = "application_failed";
pub(crate) const CODE_ANALYSIS_FAILED: &str = "analysis_failed";
pub(crate) const CODE_DISK_FULL: &str = "disk_full";
pub(crate) const CODE_SERVER_CRASHED: &str = "server_crashed";
pub(crate) const CODE_SERVER_OUT_OF_MEMORY: &str = "server_out_of_memory";
pub(crate) const CODE_LOCKFILE_DRIFT: &str = "lockfile_drift";
pub(crate) const CODE_STALE_GENERATED: &str = "stale_generated";
pub(crate) const CODE_FETCH_FAILED: &str = "fetch_failed";
pub(crate) const CODE_NO_TOOLCHAIN: &str = "no_toolchain";
pub(crate) const CODE_MODULE_NOT_FOUND: &str = "module_not_found";

/// The class of one failure, reported as the `cause` of its `error` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Input,
    Defaults,
    QueryLaunch,
    Query,
    Analysis,
    Bazel,
    Test,
    Application,
    ToolUnavailable,
    Collection,
    Report,
    Host,
    Operational,
    Unknown,
}

impl Cause {
    pub fn name(self) -> &'static str {
        match self {
            Cause::Input => "input",
            Cause::Defaults => "defaults",
            Cause::QueryLaunch => "query_launch",
            Cause::Query => "query",
            Cause::Analysis => "analysis",
            Cause::Bazel => "bazel",
            Cause::Test => "test",
            Cause::Application => "application",
            Cause::ToolUnavailable => "tool_unavailable",
            Cause::Collection => "collection",
            Cause::Report => "report",
            Cause::Host => "host",
            Cause::Operational => "operational",
            Cause::Unknown => "unknown",
        }
    }

    /// The cause an error code belongs to.
    pub fn of(code: &str) -> Cause {
        match code {
            "invalid_arguments"
            | "invalid_scope"
            | "scope_error"
            | "no_owner"
            | "no_tests"
            | "no_runnable"
            | "ambiguous_runnable"
            | "not_deployable"
            | "completion_failed"
            | "unresolved_workspace"
            | "unreadable_working_directory" => Cause::Input,
            "invalid_defaults" => Cause::Defaults,
            "query_launch_failed" => Cause::QueryLaunch,
            "query_failed" => Cause::Query,
            "analysis_failed" => Cause::Analysis,
            "bazel_failed" | "lockfile_drift" | "stale_generated" | "fetch_failed"
            | "no_toolchain" | "module_not_found" => Cause::Bazel,
            "tests_failed" => Cause::Test,
            "application_failed" => Cause::Application,
            "launch_failed" | "unsupported_platform" => Cause::ToolUnavailable,
            "unreadable_bep" | "invalid_bep" | "collection_failed" | "invalid_result" => {
                Cause::Collection
            }
            "report_failed" => Cause::Report,
            "disk_full" | "server_crashed" | "server_out_of_memory" => Cause::Host,
            "bazel_signalled" => Cause::Unknown,
            _ => Cause::Operational,
        }
    }
}

/// One failure on its way to stderr and, in JSON mode, the terminal event pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: String,
    pub message: String,
    pub exit: i32,
    pub status: Option<i32>,
    pub excerpt: Option<Excerpt>,
    pub phase: Option<&'static str>,
    pub usage: bool,
}

impl Failure {
    pub fn new(code: &str, message: &str, exit: i32) -> Self {
        Failure {
            code: code.to_owned(),
            message: message.to_owned(),
            exit,
            status: None,
            excerpt: None,
            phase: None,
            usage: true,
        }
    }

    pub fn without_usage(mut self) -> Self {
        self.usage = false;
        self
    }

    pub fn cause(&self) -> Cause {
        Cause::of(&self.code)
    }

    fn shows_usage(&self) -> bool {
        self.usage
            && matches!(
                self.code.as_str(),
                "invalid_arguments" | "invalid_defaults" | "completion_failed"
            )
            && self.exit == pre_exec_code()
    }

    /// The failure of scope resolution, keeping a query's native status and excerpt.
    pub(crate) fn resolve(error: &ResolveError) -> Self {
        let (code, exit) = match error {
            ResolveError::QueryFailed {
                failure: QueryFailure::Launch { .. },
                ..
            } => (CODE_QUERY_LAUNCH_FAILED, operational_code()),
            ResolveError::QueryFailed {
                failure: QueryFailure::Prepare { .. },
                ..
            } => (CODE_INVALID_SCOPE, pre_exec_code()),
            ResolveError::QueryFailed {
                failure: QueryFailure::DiskFull { status, .. },
                ..
            } => (CODE_DISK_FULL, *status),
            ResolveError::QueryFailed { .. } => (CODE_QUERY_FAILED, pre_exec_code()),
            ResolveError::NoOwner { .. } => (CODE_NO_OWNER, pre_exec_code()),
            ResolveError::NoTests { .. } => (CODE_NO_TESTS, operational_code()),
            ResolveError::NoRunnable { .. } => ("no_runnable", operational_code()),
            ResolveError::AmbiguousRunnable { .. } => ("ambiguous_runnable", operational_code()),
            ResolveError::NotDeployable { .. } => (CODE_NOT_DEPLOYABLE, pre_exec_code()),
            ResolveError::Config { .. } => ("invalid_config", pre_exec_code()),
            _ => (CODE_INVALID_SCOPE, pre_exec_code()),
        };
        let mut failure = Failure::new(code, &error.to_string(), exit);
        failure.phase = Some("resolve");
        match error {
            ResolveError::QueryFailed {
                failure: QueryFailure::Exit { status, excerpt },
                ..
            } => {
                failure.status = *status;
                failure.excerpt = excerpt.clone();
            }
            ResolveError::QueryFailed {
                failure:
                    QueryFailure::DiskFull {
                        status, excerpt, ..
                    },
                ..
            } => {
                failure.status = Some(*status);
                failure.excerpt = excerpt.clone();
            }
            _ => {}
        }
        failure
    }
}

fn next_hint(failure: &Failure) -> Option<String> {
    super::bazel_hint::recorded()
        .filter(|hint| hint.code == failure.code)
        .map(|hint| hint.next)
        .or_else(|| super::guidance::next_action(&failure.code).map(str::to_owned))
}

/// The printed next step without its `next: ` prefix, for the `error` event.
fn next_step(failure: &Failure) -> Option<String> {
    if failure.shows_usage() {
        return None;
    }
    let hint = next_hint(failure)?;
    Some(hint.strip_prefix("next: ").unwrap_or(&hint).to_owned())
}

/// Builds the `error` event of one failure; an event that fails to build becomes `invalid_result`.
pub(crate) fn error_event_of(failure: &Failure) -> Result<serde_json::Value, i32> {
    let cause = failure.cause();
    let next = next_step(failure);
    let facts = FailureFacts {
        cause: cause.name(),
        status: failure.status,
        excerpt: failure.excerpt.as_ref(),
        next: next.as_deref(),
    };
    failure_event(&failure.code, &failure.message, failure.phase, &facts)
        .or_else(|error| {
            failure_event(
                CODE_INVALID_RESULT,
                &error.to_string(),
                failure.phase,
                &FailureFacts {
                    cause: Cause::of(CODE_INVALID_RESULT).name(),
                    status: failure.status,
                    excerpt: None,
                    next: None,
                },
            )
        })
        .map_err(|error| stdout_output_code(&error))
}

/// Writes the `error` event and the terminal `command_finished` for one failure.
pub fn emit_failure_events(out: &mut dyn Write, failure: &Failure) -> Result<(), i32> {
    emit_event(out, &error_event_of(failure)?)?;
    emit_event(
        out,
        &command_finished(
            failure.exit,
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        ),
    )?;
    flush_out(out)
}

static USAGE_COMMAND: std::sync::OnceLock<dx_plan::args::Command> = std::sync::OnceLock::new();

/// Records the command whose usage a later argument error prints.
pub fn set_usage_command(command: dx_plan::args::Command) {
    let _ = USAGE_COMMAND.set(command);
}

fn usage_text() -> String {
    match USAGE_COMMAND.get() {
        Some(command) => command.usage().to_owned(),
        None => dx_plan::args::help::usage_banner(),
    }
}

/// Writes the cancellation line for a stop signal, closes the JSON stream, and returns its exit code.
pub fn cancel(output: OutputMode, out: &mut dyn Write, err: &mut dyn Write, signo: i32) -> i32 {
    let _ = writeln!(err, "dx: cancelled: stopped by signal {signo}");
    let exit = interrupted_code(signo);
    if output == OutputMode::Json {
        let failure =
            Failure::new("cancelled", &format!("stopped by signal {signo}"), exit).without_usage();
        if let Err(code) = emit_failure_events(out, &failure) {
            return code;
        }
    }
    exit
}

/// Ends a run that could not take a build slot: a stop signal cancels, a busy wait fails with `slots_busy`.
pub fn slot_stop(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    stop: dx_plan::slots::Stop,
) -> i32 {
    match stop {
        dx_plan::slots::Stop::Signal(signo) => cancel(output, out, err, signo),
        dx_plan::slots::Stop::Busy(holder) => {
            let message = format!("no build slot freed in time (held by {holder})");
            let failure =
                Failure::new(dx_plan::slots::SLOTS_BUSY_CODE, &message, 1).without_usage();
            report_failure(output, out, err, &failure)
        }
    }
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let text = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_owned());
    match info.location() {
        Some(at) => format!("dx panicked at {}:{}: {text}", at.file(), at.line()),
        None => format!("dx panicked: {text}"),
    }
}

fn write_panic_events(out: &mut dyn Write, message: &str) {
    let failure = Failure::new("internal_error", message, 101).without_usage();
    let _ = emit_failure_events(out, &failure);
}

/// Makes a panic on the main thread close the JSON stream with `internal_error`.
pub fn install_panic_events() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") {
            write_panic_events(&mut std::io::stdout().lock(), &panic_message(info));
        }
        previous(info);
    }));
}

fn write_panic_hint(err: &mut dyn Write) {
    let _ = writeln!(err, "dx: {}", dx_process::BUG_REPORT_HINT);
}

/// Prints where to report a bug after the default panic message.
pub fn install_panic_hint() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        write_panic_hint(&mut std::io::stderr().lock());
    }));
}

/// Writes one failure to stderr and, in JSON mode, to stdout; a stop signal turns it into a cancellation.
pub fn report_failure(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    failure: &Failure,
) -> i32 {
    if let Some(signo) = interrupted() {
        return cancel(output, out, err, signo);
    }
    if failure.shows_usage() || failure.code == CODE_INVALID_ARGUMENTS {
        let _ = writeln!(err, "dx: {}", failure.message);
    } else {
        let _ = writeln!(err, "dx: {}: {}", failure.code, failure.message);
    }
    if let Some(excerpt) = &failure.excerpt {
        for line in excerpt.text.lines() {
            let _ = writeln!(err, "  {line}");
        }
        if excerpt.truncated {
            let _ = writeln!(err, "  (excerpt truncated)");
        }
    }
    if failure.shows_usage() {
        let _ = writeln!(err, "{}", usage_text());
    } else if let Some(next) = next_hint(failure) {
        let _ = writeln!(err, "{next}");
    }
    if output == OutputMode::Json
        && let Err(exit) = emit_failure_events(out, failure)
    {
        return exit;
    }
    failure.exit
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_panic_hint_names_the_bug_template_and_dx_version() {
        let mut err = Vec::new();
        super::write_panic_hint(&mut err);
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("issues/new?template=bug_report.md"));
        assert!(text.contains("dx version"));
    }

    #[test]
    fn the_error_event_carries_the_next_step_of_a_known_code() {
        let failure = super::Failure::new("disk_full", "no space", 1);
        let event = super::error_event_of(&failure).unwrap();
        let next = event["next"].as_str().unwrap();
        assert!(next.starts_with("free disk space"), "{next}");
        let plain = super::Failure::new("no_such_code", "x", 1);
        assert!(super::error_event_of(&plain).unwrap().get("next").is_none());
    }

    #[test]
    fn cancel_names_the_signal_and_closes_the_json_stream() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::cancel(OutputMode::Json, &mut out, &mut err, 15);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "dx: cancelled: stopped by signal 15\n"
        );
        assert_eq!(code, 128 + 15);
        let events = String::from_utf8(out).unwrap();
        let kinds: Vec<_> = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect();
        assert_eq!(kinds[0]["event"], "error");
        assert_eq!(kinds[0]["code"], "cancelled");
        assert_eq!(kinds[1]["event"], "command_finished");
        assert_eq!(kinds[1]["exit_code"], 143);
    }

    #[test]
    fn a_busy_slot_wait_fails_with_slots_busy_and_closes_the_json_stream() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let stop = dx_plan::slots::Stop::Busy("pid 7".to_owned());
        let code = super::slot_stop(OutputMode::Json, &mut out, &mut err, stop);
        assert_eq!(code, 1);
        let events: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events[0]["event"], "error");
        assert_eq!(events[0]["code"], "slots_busy");
        assert!(events[0]["message"].as_str().unwrap().contains("pid 7"));
        assert_eq!(events[1]["event"], "command_finished");
        assert_eq!(events[1]["exit_code"], 1);
    }

    #[test]
    fn cancel_in_text_mode_writes_nothing_to_stdout() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            super::cancel(OutputMode::Text { quiet: false }, &mut out, &mut err, 2),
            130
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_panic_writes_internal_error_and_exit_101() {
        let mut out = Vec::new();
        super::write_panic_events(&mut out, "dx panicked at x.rs:1: boom");
        let lines: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines[0]["event"], "error");
        assert_eq!(lines[0]["code"], "internal_error");
        assert_eq!(lines[1]["event"], "command_finished");
        assert_eq!(lines[1]["exit_code"], 101);
    }

    use super::*;
    use dx_process::{broken_pipe_code, operational_code};
    use std::io;

    struct BrokenPipe;

    impl Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_refusal_prints_one_line_and_no_usage() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = super::super::common::refusal(
            OutputMode::Text { quiet: false },
            &mut out,
            &mut err,
            "leased",
        );
        assert_eq!(exit, pre_exec_code());
        assert_eq!(String::from_utf8(err).expect("utf8"), "dx: leased\n");
    }

    #[test]
    fn a_scope_failure_prints_no_usage() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let failure = Failure::new(CODE_INVALID_SCOPE, "bad scope", pre_exec_code());
        report_failure(
            OutputMode::Text { quiet: false },
            &mut out,
            &mut err,
            &failure,
        );
        let text = String::from_utf8(err).expect("utf8");
        assert!(!text.contains("usage:"), "{text}");
        assert!(text.contains("invalid_scope"), "{text}");
    }

    fn events(out: &[u8]) -> Vec<serde_json::Value> {
        std::str::from_utf8(out)
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("NDJSON"))
            .collect()
    }

    #[test]
    fn an_unbuildable_failure_writes_one_invalid_result_and_one_finished() {
        let mut out = Vec::new();
        let failure = Failure::new(CODE_INVALID_SCOPE, "", pre_exec_code());
        assert_eq!(emit_failure_events(&mut out, &failure), Ok(()));
        let written = events(&out);
        assert_eq!(written.len(), 2, "{written:?}");
        assert_eq!(written[0]["event"], "error");
        assert_eq!(written[0]["code"], CODE_INVALID_RESULT);
        assert_eq!(written[1]["event"], "command_finished");
        assert_eq!(written[1]["exit_code"], pre_exec_code());
    }

    fn query_error(failure: QueryFailure) -> ResolveError {
        ResolveError::QueryFailed {
            label: "owners".to_owned(),
            failure,
        }
    }

    #[test]
    fn every_code_has_one_cause_and_input_classes_stay_separate() {
        for (code, cause) in [
            ("invalid_arguments", "input"),
            ("invalid_scope", "input"),
            ("no_owner", "input"),
            ("invalid_defaults", "defaults"),
            ("query_launch_failed", "query_launch"),
            ("query_failed", "query"),
            ("analysis_failed", "analysis"),
            ("bazel_failed", "bazel"),
            ("tests_failed", "test"),
            ("application_failed", "application"),
            ("launch_failed", "tool_unavailable"),
            ("invalid_bep", "collection"),
            ("collection_failed", "collection"),
            ("report_failed", "report"),
            ("disk_full", "host"),
            ("bazel_signalled", "unknown"),
        ] {
            assert_eq!(Cause::of(code).name(), cause, "{code}");
        }
    }

    #[test]
    fn disk_full_from_a_query_has_host_cause_and_exit_36() {
        let error = query_error(QueryFailure::DiskFull {
            status: 36,
            path: std::path::PathBuf::from("/ob"),
            free: 1 << 20,
            excerpt: None,
        });
        let failure = Failure::resolve(&error);
        assert_eq!(failure.code, CODE_DISK_FULL);
        assert_eq!(failure.exit, 36);
        assert_eq!(failure.status, Some(36));
        assert_eq!(failure.phase, Some("resolve"));
        assert_eq!(failure.cause().name(), "host");
    }

    #[test]
    fn resolve_failures_keep_the_query_cause_status_and_excerpt() {
        let excerpt = Excerpt {
            text: "ERROR: no such package".to_owned(),
            truncated: true,
        };
        let failure = Failure::resolve(&query_error(QueryFailure::Exit {
            status: Some(7),
            excerpt: Some(excerpt.clone()),
        }));
        assert_eq!(failure.code, "query_failed");
        assert_eq!(failure.status, Some(7));
        assert_eq!(failure.excerpt, Some(excerpt));
        assert_eq!(failure.exit, pre_exec_code());
        let launch = Failure::resolve(&query_error(QueryFailure::Launch {
            detail: "not found".to_owned(),
        }));
        assert_eq!(launch.code, "query_launch_failed");
        assert_eq!(launch.status, None);
        assert_eq!(launch.exit, operational_code());
        let scope = Failure::resolve(&ResolveError::PathNotFound {
            scope: "x.py".to_owned(),
        });
        assert_eq!(scope.code, "invalid_scope");
        assert_eq!(scope.cause(), Cause::Input);
        assert_eq!(scope.exit, pre_exec_code());
    }

    #[test]
    fn resolve_failures_split_operational_from_scope_exits() {
        let operational = operational_code();
        let usage = pre_exec_code();
        assert_ne!(operational, usage);
        for (error, code, exit) in [
            (ResolveError::EmptyScope, "invalid_scope", usage),
            (
                ResolveError::PathNotFound {
                    scope: "x".to_owned(),
                },
                "invalid_scope",
                usage,
            ),
            (
                ResolveError::NoOwner {
                    files: vec!["a".to_owned()],
                    labels: vec![],
                },
                "no_owner",
                usage,
            ),
            (
                ResolveError::NoTests {
                    owners: vec!["//a:lib".to_owned()],
                },
                "no_tests",
                operational,
            ),
            (
                ResolveError::NoRunnable {
                    scopes: vec!["a".to_owned()],
                },
                "no_runnable",
                operational,
            ),
            (
                ResolveError::AmbiguousRunnable {
                    candidates: vec!["//a:one".to_owned(), "//a:two".to_owned()],
                },
                "ambiguous_runnable",
                operational,
            ),
            (
                query_error(QueryFailure::Launch {
                    detail: "x".to_owned(),
                }),
                "query_launch_failed",
                operational,
            ),
            (
                query_error(QueryFailure::Exit {
                    status: Some(1),
                    excerpt: None,
                }),
                "query_failed",
                usage,
            ),
        ] {
            let failure = Failure::resolve(&error);
            assert_eq!(
                (failure.code.as_str(), failure.exit),
                (code, exit),
                "{error}"
            );
        }
    }

    #[test]
    fn json_failures_emit_the_error_then_the_terminal_event() {
        let failure = Failure::resolve(&query_error(QueryFailure::Exit {
            status: Some(7),
            excerpt: Some(Excerpt {
                text: "Loading: 1 package\nERROR: boom".to_owned(),
                truncated: false,
            }),
        }));
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = report_failure(OutputMode::Json, &mut out, &mut err, &failure);
        assert_eq!(code, 2);
        let events = events(&out);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "error");
        assert_eq!(events[0]["code"], "query_failed");
        assert_eq!(events[0]["cause"], "query");
        assert_eq!(events[0]["status"], 7);
        assert_eq!(events[0]["phase"], "resolve");
        assert_eq!(events[0]["excerpt"], "Loading: 1 package\nERROR: boom");
        assert_eq!(events[0]["truncated"], false);
        assert_eq!(events[1]["event"], "command_finished");
        assert_eq!(events[1]["exit_code"], 2);
        assert_eq!(events[1]["results_complete"], false);
        let err = String::from_utf8(err).expect("utf8");
        assert!(
            err.starts_with("dx: query_failed: query owners failed: Bazel exited 7\n  Loading: 1 package\n  ERROR: boom\n"),
            "{err}"
        );
        assert!(!err.contains("usage:"), "{err}");
    }

    #[test]
    fn scope_failures_show_one_line_and_text_mode_keeps_stdout_empty() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let failure = Failure::new("invalid_scope", "unknown path", pre_exec_code());
        let code = report_failure(
            OutputMode::Text { quiet: false },
            &mut out,
            &mut err,
            &failure,
        );
        assert_eq!(code, 2);
        assert!(out.is_empty());
        let err = String::from_utf8(err).expect("utf8");
        assert_eq!(
            err,
            "dx: invalid_scope: unknown path\nnext: check the target or path scope and run the command again.\n"
        );
    }

    #[test]
    fn a_truncated_excerpt_is_marked_on_stderr() {
        let mut failure = Failure::new("query_failed", "query failed", pre_exec_code());
        failure.excerpt = Some(Excerpt {
            text: "ERROR: tail".to_owned(),
            truncated: true,
        });
        let mut err = Vec::new();
        report_failure(
            OutputMode::Text { quiet: true },
            &mut Vec::new(),
            &mut err,
            &failure,
        );
        assert!(String::from_utf8(err)
            .expect("utf8")
            .ends_with(
            "  ERROR: tail\n  (excerpt truncated)\nnext: check the scope in the query error above and run the command again.\n"
        ));
    }

    #[test]
    fn a_closed_stdout_ends_the_failure_with_the_broken_pipe_exit() {
        let failure = Failure::new("collection_failed", "boom", operational_code());
        let code = report_failure(OutputMode::Json, &mut BrokenPipe, &mut Vec::new(), &failure);
        assert_eq!(code, broken_pipe_code());
    }
}
