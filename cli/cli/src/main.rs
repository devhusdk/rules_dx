#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use dx_cli::exec::common::flush_out;
use dx_cli::{execute, Env};
use dx_output::OutputMode;
use dx_plan::args::{
    early_workspace_flag, is_discovery_exempt, is_help_request, json_output_intent,
    load_startup_defaults, parse_with_ci, DX_WORKSPACE_ENV,
};
use dx_plan::plan::{create_run_temp_dir, remove_on_stop};
use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_process::lifecycle::{
    self, CleanupBudget, EnvPolicy, StreamGroup, StreamOutcome, StreamOutput, StreamSpec,
    StreamStdin,
};
use dx_process::{
    discover_real, operational_code, pre_exec_code, stdout_io_code, ChildStatus, Runner,
};

const STREAM_CLEANUP: CleanupBudget = CleanupBudget {
    graceful: Duration::from_secs(10),
    forced: Duration::from_secs(2),
    drain: Duration::from_secs(5),
};

#[cfg(unix)]
extern "C" fn forward_to_child(
    signo: libc::c_int,
    info: *mut libc::siginfo_t,
    _context: *mut libc::c_void,
) {
    let origin = if info.is_null() {
        lifecycle::StopOrigin::Process
    } else {
        unsafe { lifecycle::stop_origin(signo, (*info).si_code) }
    };
    lifecycle::forward_stop(signo, origin);
}

#[cfg(unix)]
fn install_forwarding() {
    for signo in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = forward_to_child as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut inherited: libc::sigaction = std::mem::zeroed();
            libc::sigaction(signo, &action, &mut inherited);
            if signo == libc::SIGHUP && inherited.sa_sigaction == libc::SIG_IGN {
                libc::sigaction(signo, &inherited, std::ptr::null_mut());
            }
        }
    }
}

#[cfg(not(unix))]
fn install_forwarding() {}

#[cfg(unix)]
fn die_by_signal(signo: i32) {
    if signo > 0 {
        let _cleaned = dx_plan::plan::clean_for_stop();
        unsafe {
            libc::signal(signo, libc::SIG_DFL);
            libc::raise(signo);
        }
    }
}

#[cfg(not(unix))]
fn die_by_signal(_signo: i32) {}

struct BinaryRunner {
    inherit_stdout: bool,
}

fn stream_spec(
    argv: &[String],
    cwd: &Path,
    env: &[(&str, &str)],
    clear_env: bool,
    inherit_stdout: bool,
) -> StreamSpec {
    let vars = env
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect();
    StreamSpec {
        argv: argv.iter().map(Into::into).collect(),
        cwd: cwd.to_path_buf(),
        env: if clear_env {
            EnvPolicy::Controlled { vars }
        } else {
            EnvPolicy::Inherited { extra: vars }
        },
        stdin: StreamStdin::Inherit,
        stdout: if inherit_stdout {
            StreamOutput::Inherit
        } else {
            StreamOutput::Forward(Box::new(io::stderr()))
        },
        stderr: StreamOutput::Inherit,
        group: stream_group(),
        held: Vec::new(),
    }
}

fn stream_group() -> StreamGroup {
    if lifecycle::terminal_foreground() {
        StreamGroup::Terminal
    } else {
        StreamGroup::Owned
    }
}

fn child_status(outcome: StreamOutcome) -> Option<ChildStatus> {
    match outcome {
        StreamOutcome::Exited(exit) => Some(exit.into()),
        StreamOutcome::Cancelled { signal } => {
            die_by_signal(signal);
            Some(ChildStatus::signalled(None))
        }
        StreamOutcome::Withdrawn => None,
    }
}

fn spawn_streamed(
    argv: &[String],
    cwd: &Path,
    env: &[(&str, &str)],
    clear_env: bool,
    inherit_stdout: bool,
) -> io::Result<ChildStatus> {
    let spec = stream_spec(argv, cwd, env, clear_env, inherit_stdout);
    let outcome = lifecycle::run_streamed(spec, STREAM_CLEANUP, &dx_process::interrupted)?;
    Ok(child_status(outcome).unwrap_or(ChildStatus::signalled(None)))
}

impl Runner for BinaryRunner {
    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        spawn_streamed(argv, cwd, env, true, self.inherit_stdout)
    }

    fn gitleaks_tool(&self) -> Option<std::path::PathBuf> {
        std::env::var_os("DX_GITLEAKS_BIN")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
    }

    fn pinned_tool(&self, var: &str) -> Option<std::path::PathBuf> {
        dx_process::pinned_env_path(var)
    }

    fn tool_version(&self, path: &Path) -> Option<String> {
        dx_process::version_line(path)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        dx_process::disk_space(path).map(|space| space.free)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        spawn_streamed(argv, cwd, env, false, self.inherit_stdout)
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        let mut spec = stream_spec(argv, cwd, env, false, self.inherit_stdout);
        spec.stdin = StreamStdin::Null;
        spec.group = StreamGroup::Owned;
        let outcome = lifecycle::run_streamed_until(
            spec,
            STREAM_CLEANUP,
            &dx_process::interrupted,
            withdraw,
        )?;
        Ok(child_status(outcome))
    }
}

#[derive(Clone, Default)]
struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Captured {
    fn take(&self) -> Vec<u8> {
        std::mem::take(
            &mut *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BinaryQueryRunner;

impl QueryRunner for BinaryQueryRunner {
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        dx_process::disk_space(path).map(|space| space.free)
    }

    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        let stdout = Captured::default();
        let stderr = Captured::default();
        let spec = StreamSpec {
            argv: argv.iter().map(Into::into).collect(),
            cwd: cwd.to_path_buf(),
            env: EnvPolicy::Inherited { extra: Vec::new() },
            stdin: StreamStdin::Null,
            stdout: StreamOutput::Forward(Box::new(stdout.clone())),
            stderr: StreamOutput::Forward(Box::new(stderr.clone())),
            group: stream_group(),
            held: Vec::new(),
        };
        let outcome = lifecycle::run_streamed(spec, STREAM_CLEANUP, &dx_process::interrupted)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::InvalidInput {
                    io::Error::new(io::ErrorKind::InvalidInput, "query needs a binary")
                } else {
                    error
                }
            })?;
        Ok(QueryResult {
            code: child_status(outcome).and_then(|status| status.code),
            stdout: stdout.take(),
            stderr: stderr.take(),
        })
    }
}

fn startup_failure(stderr_text: &str, code: &str, message: &str, exit: i32, json: bool) -> i32 {
    let _ = writeln!(io::stderr(), "{stderr_text}");
    if !json {
        return exit;
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if let Err(io_exit) = dx_cli::emit_startup_outcome(&mut out, code, message, exit) {
        return io_exit;
    }
    exit
}

fn usage_error(
    code: &str,
    message: &str,
    json: bool,
    command: Option<dx_plan::args::Command>,
) -> i32 {
    let usage = command.map_or_else(dx_plan::args::help::usage_banner, |c| c.usage().to_owned());
    startup_failure(
        &format!("dx: {message}\n{usage}"),
        code,
        message,
        pre_exec_code(),
        json,
    )
}

fn main() {
    install_forwarding();
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    // LCOV_EXCL_START - reason: thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let Some(kind) = dx_cli::hook_kind(&args) {
        let hook_cwd =
            dx_process::current_directory().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let code = dx_cli::run_worktree_hook(kind, &hook_cwd, &mut out, &mut io::stderr());
        return match flush_out(&mut out) {
            Ok(()) => code,
            Err(exit) => exit,
        };
    }
    let env_get = |name: &str| std::env::var(name).ok();
    let is_ci = dx_process::is_ci();
    let bare_intent = json_output_intent(
        &args,
        &env_get,
        &dx_plan::args::FileDefaults::default(),
        is_ci,
    );
    match dx_plan::args::try_complete() {
        Ok(true) => return 0,
        Ok(false) => {}
        Err(message) => {
            return startup_failure(
                &format!("dx: {message}\n{}", dx_plan::args::help::usage_banner()),
                "completion_failed",
                &message,
                pre_exec_code(),
                bare_intent,
            );
        }
    }
    let initial_cwd =
        dx_process::current_directory().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let initial_start = dx_process::workspace_start(&initial_cwd);
    let flag_workspace = early_workspace_flag(&args);
    let env_workspace = std::env::var(DX_WORKSPACE_ENV)
        .ok()
        .filter(|value| !value.is_empty())
        .filter(|_| !dx_plan::args::ci_ignores_env(DX_WORKSPACE_ENV, is_ci));
    let file_defaults = match load_startup_defaults(
        &initial_start,
        &initial_cwd,
        flag_workspace,
        env_workspace,
        is_ci,
    ) {
        Ok(defaults) => defaults,
        Err(failure) => {
            if is_help_request(&args) {
                *failure.partial
            } else {
                let json = json_output_intent(&args, &env_get, &failure.partial, is_ci);
                return startup_failure(
                    &format!("dx: {}", failure.detail),
                    "invalid_defaults",
                    &failure.detail,
                    pre_exec_code(),
                    json,
                );
            }
        }
    };
    let mut invocation = match parse_with_ci(&args, &env_get, &file_defaults, is_ci) {
        Ok(invocation) => invocation,
        Err(dx_plan::args::ArgsError::Help { text }) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if let Err(error) = write!(out, "{text}") {
                return stdout_io_code(&error);
            }
            if let Err(exit) = flush_out(&mut out) {
                return exit;
            }
            return 0;
        }
        Err(error) => {
            let json = json_output_intent(&args, &env_get, &file_defaults, is_ci);
            let code = match error {
                dx_plan::args::ArgsError::BadDefault { .. } => "invalid_defaults",
                _ => "invalid_arguments",
            };
            let command = dx_plan::args::help::help_command_in(&args);
            return usage_error(code, &error.to_string(), json, command);
        }
    };
    // LCOV_EXCL_STOP - reason: end thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    dx_cli::set_usage_command(invocation.command);
    dx_output::init_diagnostics_with_color(invocation.verbose, invocation.color);
    tracing::info!(
        command = invocation.command.name(),
        verbose = invocation.verbose,
        quiet = invocation.quiet,
        "dx invocation parsed"
    );
    if let Some(message) = dx_plan::platform::host_refusal() {
        return startup_failure(
            &format!("dx: {message}"),
            "unsupported_platform",
            &message,
            operational_code(),
            invocation.output == OutputMode::Json,
        );
    }
    let cwd = match dx_process::current_directory() {
        Ok(cwd) => cwd,
        Err(error) => {
            let message = format!("cannot read working directory: {error}");
            return startup_failure(
                &format!("dx: {message}"),
                "unreadable_working_directory",
                &message,
                pre_exec_code(),
                invocation.output == OutputMode::Json,
            );
        }
    };
    let start = dx_process::workspace_start(&cwd);
    let override_dir = invocation
        .workspace
        .as_deref()
        .map(|raw| dx_process::resolve_override_display(Path::new(raw), &cwd));
    let workspace = match discover_real(&start, override_dir.as_deref()) {
        Ok(workspace) => workspace,
        Err(error) => {
            if is_discovery_exempt(invocation.command) {
                override_dir
                    .map(|dir| dx_process::canonicalize_or_keep(&dir))
                    .unwrap_or(start)
            } else {
                let message = format!("cannot resolve workspace: {error}");
                return startup_failure(
                    &format!("dx: {message}"),
                    "unresolved_workspace",
                    &message,
                    pre_exec_code(),
                    invocation.output == OutputMode::Json,
                );
            }
        }
    };
    dx_process::configure_real(&workspace);
    dx_plan::plan::set_rules_dx_repo(&workspace);
    if invocation.here {
        match dx_plan::args::apply_here(&invocation, &workspace, &cwd) {
            Ok(resolved) => invocation = resolved,
            Err(detail) => {
                return startup_failure(
                    &format!("dx: {detail}"),
                    "invalid_scope",
                    &detail,
                    pre_exec_code(),
                    invocation.output == OutputMode::Json,
                );
            }
        }
    }
    let pin = dx_plan::skew::read_pin(&workspace);
    match dx_plan::skew::disposition(&invocation, dx_plan::skew::is_skewed(&pin)) {
        dx_plan::skew::SkewDisposition::Proceed => {}
        dx_plan::skew::SkewDisposition::Warn => {
            let _ = writeln!(
                io::stderr(),
                "dx: warning: {} (proceeding: read-only invocation)",
                dx_plan::skew::diagnostic(&pin)
            );
        }
        dx_plan::skew::SkewDisposition::Refuse => {
            let message = dx_plan::skew::diagnostic(&pin);
            return startup_failure(
                &format!("dx: {message}"),
                "version_skew",
                &message,
                operational_code(),
                invocation.output == OutputMode::Json,
            );
        }
    }
    let pid = std::process::id();
    let temp_base = std::env::temp_dir();
    let (temp_dir, nonce) = match create_run_temp_dir(&temp_base, pid) {
        Ok(run) => {
            remove_on_stop(run.0.path());
            run
        }
        Err(error) => {
            let message = format!("cannot create temporary directory: {error}");
            return startup_failure(
                &format!("dx: {message}"),
                "unwritable_temporary_directory",
                &message,
                pre_exec_code(),
                invocation.output == OutputMode::Json,
            );
        }
    };
    let inherit_stdout = matches!(invocation.output, OutputMode::Text { .. })
        && !invocation
            .reports
            .iter()
            .any(|report| report.destination == "-");
    if workspace.join("MODULE.bazel").is_file() {
        dx_process::groups::set_lease_root(&workspace.join(".dx"));
    }
    dx_plan::plan::remove_stale_run_dirs(&temp_base);
    let reaped = dx_process::groups::reap_orphans();
    let runner = BinaryRunner { inherit_stdout };
    let query_runner = BinaryQueryRunner;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut err = io::stderr();
    let mut pending = Vec::new();
    for group in &reaped {
        let message = dx_process::groups::reaped_message(group);
        let _ = writeln!(err, "dx: {message}");
        if invocation.output == OutputMode::Json {
            let event = dx_output::notice_value(&dx_output::NoticeEvent {
                related_command: Some(invocation.command.name().to_owned()),
                ..dx_output::NoticeEvent::warning("orphan_process_group_stopped", message)
            });
            let _ = dx_output::lifecycle::write_event(&mut pending, &event);
        }
    }
    dx_plan::slots::inherit_share(&env_get);
    let mut slot_options = vec![invocation.profile().config_flag()];
    slot_options.extend(invocation.raw_bazel_options().iter().cloned());
    let gate = dx_plan::slots::Gate::from_env(
        &workspace,
        &env_get,
        invocation.output == OutputMode::Json,
        &slot_options,
        invocation.build_slots,
    );
    let slot = match gate.as_ref().map_or(Ok(None), |gate| {
        gate.hold(invocation.command, &mut err, &mut pending)
    }) {
        Ok(slot) => slot,
        Err(signo) => {
            let _ = writeln!(err, "dx: cancelled: stopped by signal {signo}");
            return dx_process::interrupted_code(signo);
        }
    };
    let gate = if let Some(slot) = &slot {
        std::env::set_var(dx_plan::slots::SLOT_ENV_VAR, "1");
        if let Some(share) = &slot.share {
            std::env::set_var(dx_plan::slots::SHARE_ENV_VAR, share.to_env());
        }
        None
    } else {
        gate
    };
    let mut after_first = dx_plan::slots::AfterFirstLine::new(&mut out, pending);
    let audit_today = std::env::var(dx_cli::exec::AUDIT_TODAY_ENV).ok();
    let code = dx_plan::slots::with_gate(gate, || {
        execute(
            &invocation,
            Env {
                workspace: &workspace,
                runner: &runner,
                query_runner: &query_runner,
                temp_dir: temp_dir.path(),
                pid,
                nonce,
                out: &mut after_first,
                err: &mut err,
                ci: dx_process::is_ci(),
                audit_today: audit_today.as_deref(),
            },
        )
    });
    let temp_display = temp_dir.path().display().to_string();
    if let Err(error) = temp_dir.close() {
        let _ = writeln!(
            io::stderr(),
            "dx: warning: cannot remove temporary directory {temp_display}: {error}",
        );
    }
    code
}
