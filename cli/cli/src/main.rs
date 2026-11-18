#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok,
    clippy::wildcard_enum_match_arm
)]

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use dx_cli::{ActionTally, Env, execute, print_actions, print_cache_retries};
use dx_exec_core::bazel_restart::{RestartingQueryRunner, RestartingRunner};
use dx_exec_core::common::flush_out;
use dx_output::{OutputMode, write_stderr};
use dx_plan::args::{is_discovery_exempt, json_output_intent, parse_with_ci};
use dx_plan::plan::{create_run_temp_dir, remove_on_stop};
use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_process::lifecycle::{self, EnvPolicy, StreamOutput, StreamSpec, StreamStdin};
use dx_process::{discover_real, operational_code, pre_exec_code, stdout_io_code};

#[cfg(unix)]
#[expect(
    unsafe_code,
    reason = "a signal handler saves and restores errno through a raw pointer"
)]
extern "C" fn forward_to_child(signo: libc::c_int) {
    let errno = dx_process::errno_slot();
    // SAFETY: `errno_slot` returns this thread's valid errno location.
    let saved = unsafe { *errno };
    lifecycle::forward_stop(signo);
    // SAFETY: same location as above, still valid on this thread.
    unsafe { *errno = saved };
}

#[cfg(unix)]
#[expect(unsafe_code, reason = "sigaction has no safe std API")]
fn install_forwarding() {
    for signo in lifecycle::STOP_SIGNALS {
        // SAFETY: zeroed `sigaction` is valid; the handler only calls async-signal-safe functions.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = forward_to_child as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&raw mut action.sa_mask);
            let mut inherited: libc::sigaction = std::mem::zeroed();
            libc::sigaction(signo, &raw const action, &raw mut inherited);
            if signo == libc::SIGHUP && inherited.sa_sigaction == libc::SIG_IGN {
                libc::sigaction(signo, &raw const inherited, std::ptr::null_mut());
            }
        }
    }
}

#[cfg(windows)]
fn install_forwarding() {
    lifecycle::install_console_stop();
}

#[cfg(not(any(unix, windows)))]
fn install_forwarding() {}

#[cfg(unix)]
#[expect(unsafe_code, reason = "re-raising a signal has no safe std API")]
fn die_by_signal(signo: i32) {
    if signo > 0 {
        let _cleaned = dx_plan::plan::clean_for_stop();
        // SAFETY: restoring the default disposition and raising a signal has no memory effects.
        unsafe {
            libc::signal(signo, libc::SIG_DFL);
            libc::raise(signo);
        }
    }
}

#[cfg(not(unix))]
fn die_by_signal(_signo: i32) {}

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

struct BinaryQueryRunner {
    runner: dx_process::BinaryRunner,
}

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
            group: lifecycle::stream_group(),
            held: Vec::new(),
        };
        let status = self.runner.stream(spec).map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidInput {
                io::Error::new(io::ErrorKind::InvalidInput, "query needs a binary")
            } else {
                error
            }
        })?;
        Ok(QueryResult {
            code: status.and_then(|status| status.code),
            stdout: stdout.take(),
            stderr: stderr.take(),
        })
    }
}

fn startup_failure(stderr_text: &str, code: &str, message: &str, exit: i32, json: bool) -> i32 {
    write_stderr(&mut io::stderr(), format_args!("{stderr_text}\n"));
    if !json {
        return exit;
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if let Err(io_exit) = dx_exec_core::common::emit_startup_outcome(&mut out, code, message, exit)
    {
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
    if let Some(code) = run_as_tool() {
        std::process::exit(code);
    }
    install_forwarding();
    dx_cli::exec::install_panic_hint();
    let code = run();
    std::process::exit(code);
}

// LCOV_EXCL_START - reason: thin multicall shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
fn run_as_tool() -> Option<i32> {
    use dx_cli::multicall::{Tool, subcommand_for, tool_for};
    let mut args = std::env::args_os().peekable();
    let argv0 = args.next()?;
    let tool = match tool_for(&argv0) {
        Some(tool) => tool,
        None => {
            let tool = subcommand_for(args.peek()?)?;
            args.next();
            tool
        }
    };
    let lossy = |args: std::iter::Peekable<std::env::ArgsOs>| -> Vec<String> {
        args.map(|arg| arg.to_string_lossy().into_owned()).collect()
    };
    Some(match tool {
        Tool::Runner => quality_runner::entry::run_cli(&lossy(args)),
        Tool::Markdown => quality_markdown::entry::run_process(&lossy(args)),
        Tool::Notebook => quality_notebook::entry::run_process(&lossy(args)),
        Tool::LspCheck => lsp_check::entry::run_process(&lossy(args)),
        Tool::CollectNative => collect_native::run_process(lossy(args)),
        Tool::EnvShardWriter => env_shard::writer::run_process(&lossy(args)),
        Tool::CodegenShardWriter => codegen_shard::writer::run_process(&lossy(args)),
        Tool::ReleaseTree => release_tree::run_process(&lossy(args)),
        Tool::Capture => capture::run_process(args),
    })
}
// LCOV_EXCL_STOP - reason: end thin multicall shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md

/// Decides `BAZEL_SH` for the Bazel calls this run makes; `dx_process` applies it per child.
fn apply_bazel_sh(invocation: &dx_plan::args::Invocation) -> Option<i32> {
    use dx_process::EnvChange;
    use dx_process::bazel_sh::{BazelSh, resolve_bazel_sh};
    if !(cfg!(windows) && dx_plan::args::help::runs_bazel(invocation.command)) {
        return None;
    }
    let exists = |path: &Path| path.is_file();
    // An unreadable candidate is treated like a missing one; resolution moves to the next.
    let read = |path: &Path| std::fs::read(path).ok();
    match resolve_bazel_sh(&|name| std::env::var_os(name), &exists, &read) {
        Ok(BazelSh::Set(bash)) => {
            dx_process::init_bazel_env(vec![("BAZEL_SH", EnvChange::Set(bash.into_os_string()))]);
        }
        Ok(BazelSh::Unset) => dx_process::init_bazel_env(vec![("BAZEL_SH", EnvChange::Remove)]),
        Ok(BazelSh::Keep) => {}
        Err(error) => {
            let message = error.to_string();
            return Some(startup_failure(
                &format!("dx: {message}"),
                "wsl_bash",
                &message,
                pre_exec_code(),
                invocation.output == OutputMode::Json,
            ));
        }
    }
    None
}

fn resolve_workspace(
    invocation: &dx_plan::args::Invocation,
    cwd: &Path,
) -> Result<std::path::PathBuf, i32> {
    let start = dx_process::workspace_start(cwd);
    let override_dir = invocation
        .workspace
        .as_deref()
        .map(|raw| dx_process::resolve_override_display(Path::new(raw), cwd));
    match discover_real(&start, override_dir.as_deref()) {
        Ok(workspace) => Ok(workspace),
        Err(error) => {
            if is_discovery_exempt(invocation.command) {
                Ok(override_dir
                    .map(|dir| dx_process::canonicalize_or_keep(&dir))
                    .unwrap_or(start))
            } else {
                let message = format!("cannot resolve workspace: {error}");
                Err(startup_failure(
                    &format!("dx: {message}"),
                    "unresolved_workspace",
                    &message,
                    pre_exec_code(),
                    invocation.output == OutputMode::Json,
                ))
            }
        }
    }
}

fn make_run_dir(
    invocation: &dx_plan::args::Invocation,
    temp_base: &Path,
    pid: u32,
) -> Result<(dx_plan::plan::run_temp::RunTempDir, u64), i32> {
    match create_run_temp_dir(temp_base, pid) {
        Ok(run) => {
            remove_on_stop(run.0.path());
            Ok(run)
        }
        Err(error) => {
            let message = dx_cli::startup::temp_dir_failure(temp_base, &error);
            Err(startup_failure(
                &format!("dx: {message}"),
                "unwritable_temporary_directory",
                &message,
                pre_exec_code(),
                invocation.output == OutputMode::Json,
            ))
        }
    }
}

fn report_reaped(
    invocation: &dx_plan::args::Invocation,
    reaped: &[dx_process::groups::Reaped],
    err: &mut dyn Write,
) -> Vec<u8> {
    let mut pending = Vec::new();
    for group in reaped {
        let message = dx_process::groups::reaped_message(group);
        write_stderr(err, format_args!("dx: {message}\n"));
        if invocation.output == OutputMode::Json {
            let event = dx_output::notice_value(&dx_output::NoticeEvent {
                related_command: Some(invocation.command.name().to_owned()),
                ..dx_output::NoticeEvent::warning(
                    dx_process::groups::CODE_ORPHAN_PROCESS_GROUP_STOPPED,
                    message,
                )
            });
            drop(dx_output::lifecycle::write_event(&mut pending, &event));
        }
    }
    pending
}

// LCOV_EXCL_START - reason: thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn parse_invocation(
    env_get: &dyn Fn(&str) -> Option<String>,
    is_ci: bool,
) -> Result<dx_plan::args::Invocation, i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let Some(kind) = dx_cli::exec::hook_kind(&args) {
        let hook_cwd =
            dx_process::current_directory().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let code = dx_cli::exec::run_worktree_hook(kind, &hook_cwd, &mut out, &mut io::stderr());
        return Err(match flush_out(&mut out) {
            Ok(()) => code,
            Err(exit) => exit,
        });
    }
    let bare_intent = json_output_intent(&args, env_get, is_ci);
    match dx_plan::args::try_complete() {
        Ok(true) => return Err(0),
        Ok(false) => {}
        Err(message) => {
            return Err(startup_failure(
                &format!("dx: {message}\n{}", dx_plan::args::help::usage_banner()),
                "completion_failed",
                message.message(),
                pre_exec_code(),
                bare_intent,
            ));
        }
    }
    let mut invocation = match parse_with_ci(&args, env_get, is_ci) {
        Ok(invocation) => invocation,
        Err(dx_plan::args::ArgsError::Help { text }) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if let Err(error) = write!(out, "{text}") {
                return Err(stdout_io_code(&error));
            }
            flush_out(&mut out)?;
            return Err(0);
        }
        Err(error) => {
            let json = json_output_intent(&args, env_get, is_ci);
            let code = "invalid_arguments";
            let command = dx_plan::args::help::help_command_in(&args);
            return Err(usage_error(code, &error.to_string(), json, command));
        }
    };
    invocation.piped = !dx_output::stdout_is_terminal();
    Ok(invocation)
}
// LCOV_EXCL_STOP - reason: end thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

fn init_run(invocation: &dx_plan::args::Invocation) -> Option<i32> {
    dx_adopt::refresh_stale_pins_from_env();
    dx_cli::exec::set_usage_command(invocation.command);
    if invocation.output == OutputMode::Json {
        dx_cli::exec::install_panic_events();
    }
    dx_output::init_diagnostics_with_color(invocation.verbose, invocation.color);
    tracing::info!(
        command = invocation.command.name(),
        verbose = invocation.verbose,
        quiet = invocation.quiet,
        "dx invocation parsed"
    );
    if let Some(message) = dx_plan::platform::host_refusal() {
        return Some(startup_failure(
            &format!("dx: {message}"),
            "unsupported_platform",
            &message,
            operational_code(),
            invocation.output == OutputMode::Json,
        ));
    }
    apply_bazel_sh(invocation)
}

fn current_dir_or_fail(invocation: &dx_plan::args::Invocation) -> Result<std::path::PathBuf, i32> {
    match dx_process::current_directory() {
        Ok(cwd) => Ok(cwd),
        Err(error) => {
            let message = dx_cli::startup::working_dir_failure(&error);
            Err(startup_failure(
                &format!("dx: {message}"),
                "unreadable_working_directory",
                &message,
                pre_exec_code(),
                invocation.output == OutputMode::Json,
            ))
        }
    }
}

fn apply_here(
    invocation: &dx_plan::args::Invocation,
    workspace: &Path,
    cwd: &Path,
) -> Result<dx_plan::args::Invocation, i32> {
    dx_plan::args::apply_here(invocation, workspace, cwd).map_err(|detail| {
        startup_failure(
            &format!("dx: {detail}"),
            "invalid_scope",
            detail.message(),
            pre_exec_code(),
            invocation.output == OutputMode::Json,
        )
    })
}

fn build_gate(
    invocation: &dx_plan::args::Invocation,
    workspace: &Path,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<dx_plan::slots::Gate> {
    dx_plan::slots::inherit_share(env_get);
    let mut slot_options = vec![invocation.build_mode().config_flag()];
    slot_options.extend(invocation.raw_bazel_options().iter().cloned());
    dx_plan::slots::Gate::from_env(
        workspace,
        env_get,
        invocation.output == OutputMode::Json,
        &slot_options,
        dx_plan::slots::effective_setting(invocation.build_slots, workspace),
    )
    .map(|gate| gate.waiting_at_most(invocation.build_slots_wait))
}

fn retries_cache(invocation: &dx_plan::args::Invocation) -> bool {
    matches!(
        invocation.command,
        dx_plan::args::Command::Build
            | dx_plan::args::Command::Test
            | dx_plan::args::Command::Coverage
            | dx_plan::args::Command::Check
    ) && !invocation.no_retry
}

fn remove_run_dir(temp_dir: dx_plan::plan::run_temp::RunTempDir) {
    let temp_display = temp_dir.path().display().to_string();
    if let Err(error) = temp_dir.close() {
        write_stderr(
            &mut io::stderr(),
            format_args!("dx: warning: cannot remove temporary directory {temp_display}: {error}\n"),
        );
    }
}

fn locate_workspace(invocation: &mut dx_plan::args::Invocation) -> Result<PathBuf, i32> {
    let cwd = current_dir_or_fail(invocation)?;
    let workspace = resolve_workspace(invocation, &cwd)?;
    dx_process::configure_real_in(&workspace, dx_plan::slots::slot_idle_secs(&workspace));
    dx_plan::plan::set_rules_dx_repo(&workspace);
    if let Some(note) = dx_plan::args::bare_scope_note(invocation, &workspace, &cwd) {
        eprintln!("{note}");
    }
    if invocation.here {
        *invocation = apply_here(invocation, &workspace, &cwd)?;
    }
    Ok(workspace)
}

fn run() -> i32 {
    // An unset or non-UTF-8 variable counts as unset.
    let env_get = |name: &str| std::env::var(name).ok();
    let is_ci = dx_process::is_ci();
    let mut invocation = match parse_invocation(&env_get, is_ci) {
        Ok(invocation) => invocation,
        Err(code) => return code,
    };
    if let Some(code) = init_run(&invocation) {
        return code;
    }
    let workspace = match locate_workspace(&mut invocation) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let pid = std::process::id();
    let temp_base = std::env::temp_dir();
    let (temp_dir, nonce) = match make_run_dir(&invocation, &temp_base, pid) {
        Ok(run) => run,
        Err(code) => return code,
    };
    let inherit_stdout = matches!(invocation.output, OutputMode::Text { .. })
        && !invocation
            .reports
            .iter()
            .any(|report| report.destination == "-");
    if workspace.join("MODULE.bazel").is_file() {
        dx_process::groups::set_lease_root(&dx_path::state::local_dir(&workspace));
    }
    dx_plan::plan::remove_stale_run_dirs(&temp_base);
    let reaped = dx_process::groups::reap_orphans();
    let cache_retry = retries_cache(&invocation);
    let runner =
        RestartingRunner::new(dx_process::BinaryRunner::new(inherit_stdout, die_by_signal))
            .with_cache_retry(cache_retry);
    let bazel_query_runner = RestartingQueryRunner::new(BinaryQueryRunner {
        runner: dx_process::BinaryRunner::new(inherit_stdout, die_by_signal),
    });
    // A warm run whose BUILD inputs are unchanged reads the rule graph from disk.
    let query_runner = dx_plan::resolve::PersistentGraphQuery::new(&bazel_query_runner);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut err = io::stderr();
    let mut pending = report_reaped(&invocation, &reaped, &mut err);
    let gate = build_gate(&invocation, &workspace, &env_get);
    let slot = match gate.as_ref().map_or(Ok(None), |gate| {
        gate.hold(invocation.command, &mut err, &mut pending)
    }) {
        Ok(slot) => slot,
        Err(stop) => {
            drop(out.write_all(&pending));
            return dx_cli::exec::slot_stop(invocation.output, &mut out, &mut err, stop);
        }
    };
    let gate = if slot.is_some() { None } else { gate };
    let held = slot.as_ref().map(|slot| slot.runner(&runner));
    let slot_runner = held
        .as_ref()
        .map_or(&runner as &dyn dx_process::Runner, |held| {
            held as &dyn dx_process::Runner
        });
    dx_cli::exec::preset_sync::sync_preset_layers(&invocation, &workspace, &mut err);
    if let Some(notice) =
        dx_cli::exec::first_run::first_run_notice(&invocation, &workspace, &env_get)
    {
        write_stderr(&mut err, format_args!("{notice}\n"));
    }
    let mut after_first = dx_plan::slots::AfterFirstLine::new(&mut out, pending);
    // Unset or non-UTF-8 test overrides fall back to the defaults.
    let audit_today = std::env::var(dx_cli::exec::AUDIT_TODAY_ENV).ok();
    let edit_commit_json = std::env::var(dx_cli::exec::EDIT_COMMIT_JSON_ENV).ok();
    let actions = ActionTally::default();
    let code = dx_plan::slots::with_gate(gate, || {
        execute(
            &invocation,
            Env {
                workspace: &workspace,
                runner: slot_runner,
                query_runner: &query_runner,
                temp_dir: temp_dir.path(),
                pid,
                nonce,
                out: &mut after_first,
                err: &mut err,
                ci: dx_process::is_ci(),
                audit_today: audit_today.as_deref(),
                actions: &actions,
                proc_root: Path::new("/proc"),
                edit_commit_json: edit_commit_json.as_deref(),
            },
        )
    });
    print_actions(invocation.output, &actions, &mut err);
    print_cache_retries(invocation.output, runner.cache_retries(), &mut err);
    remove_run_dir(temp_dir);
    code
}
