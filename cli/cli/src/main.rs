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
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use dx_cli::args::{
    early_workspace_flag, freeze_workspace, is_discovery_exempt, is_help_request, parse_with,
    select_startup_defaults, DX_WORKSPACE_ENV,
};
use dx_cli::exec::common::{emit_event, flush_out};
use dx_cli::plan::create_run_temp_dir;
use dx_cli::{execute, Env, ProcessQueryRunner};
use dx_output::{command_finished, error_event, FinishedCounts, OutputMode};
use dx_process::{
    discover_real, operational_code, pre_exec_code, stdout_io_code, ChildStatus, Runner,
};

static CHILD_PID: AtomicU32 = AtomicU32::new(0);

#[cfg(unix)]
extern "C" fn forward_to_child(signo: libc::c_int) {
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid != 0 {
        unsafe {
            libc::kill(pid as libc::pid_t, signo);
        }
    }
}

#[cfg(unix)]
fn install_forwarding() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            forward_to_child as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            forward_to_child as *const () as libc::sighandler_t,
        );
    }
}

#[cfg(not(unix))]
fn install_forwarding() {}

struct BinaryRunner {
    inherit_stdout: bool,
}

fn spawn_streamed(
    argv: &[String],
    cwd: &Path,
    env: &[(&str, &str)],
    clear_env: bool,
    inherit_stdout: bool,
) -> io::Result<ChildStatus> {
    let (binary, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    let mut command = Command::new(binary);
    command.args(args).current_dir(cwd).stderr(Stdio::inherit());
    if clear_env {
        command.env_clear();
    }
    command.envs(env.iter().copied());
    if inherit_stdout {
        command.stdout(Stdio::inherit());
    } else {
        command.stdout(Stdio::piped());
    }
    let mut child = command.spawn()?;
    CHILD_PID.store(child.id(), Ordering::SeqCst);
    let pump = if inherit_stdout {
        None
    } else {
        let stdout = child.stdout.take();
        Some(std::thread::spawn(move || {
            if let Some(mut stdout) = stdout {
                let mut stderr = io::stderr();
                let _ = io::copy(&mut stdout, &mut stderr);
            }
        }))
    };
    let status = child.wait();
    CHILD_PID.store(0, Ordering::SeqCst);
    if let Some(pump) = pump {
        let _ = pump.join();
    }
    let status = status?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signo) = status.signal() {
            unsafe {
                libc::signal(signo, libc::SIG_DFL);
                libc::raise(signo);
            }
        }
    }
    Ok(ChildStatus {
        code: status.code(),
    })
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

    fn git_tool(&self) -> Option<std::path::PathBuf> {
        std::env::var_os("DX_GIT_BIN")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        spawn_streamed(argv, cwd, env, false, self.inherit_stdout)
    }
}

fn usage_error(message: &str) -> i32 {
    let _ = writeln!(
        io::stderr(),
        "dx: {message}\n{}",
        dx_cli::args::help::usage_banner()
    );
    pre_exec_code()
}

fn main() {
    install_forwarding();
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    // LCOV_EXCL_START - reason: thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match dx_cli::args::try_complete() {
        Ok(true) => return 0,
        Ok(false) => {}
        Err(message) => return usage_error(&message),
    }
    let initial_cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let initial_start = dx_process::workspace_start(&initial_cwd);
    let flag_workspace = early_workspace_flag(&args);
    let env_workspace = std::env::var(DX_WORKSPACE_ENV)
        .ok()
        .filter(|value| !value.is_empty());
    let ci = dx_process::is_ci();
    let (mut file_defaults, file_redirect) =
        match select_startup_defaults(&initial_start, flag_workspace, env_workspace, ci) {
            Ok(selected) => (selected.defaults, selected.file_workspace),
            Err(detail) => {
                if is_help_request(&args) {
                    (dx_cli::args::FileDefaults::default(), None)
                } else {
                    return usage_error(&detail);
                }
            }
        };
    if let Some(target) = file_redirect {
        let dir = dx_process::resolve_override_display(Path::new(&target), &initial_start);
        match select_startup_defaults(&dir, Some(target.clone()), None, ci) {
            Ok(selected) => {
                file_defaults = freeze_workspace(&selected.defaults, &target);
            }
            Err(detail) => {
                if !is_help_request(&args) {
                    return usage_error(&detail);
                }
            }
        }
    };
    let env_get = |name: &str| {
        if ci && dx_cli::args::is_preference_env(name) {
            None
        } else {
            std::env::var(name).ok()
        }
    };
    let mut invocation = match parse_with(&args, &env_get, &file_defaults) {
        Ok(invocation) => invocation,
        Err(dx_cli::args::ArgsError::Help { text }) => {
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
        Err(error) => return usage_error(&error.to_string()),
    };
    // LCOV_EXCL_STOP - reason: end thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    dx_output::init_diagnostics_with_color(
        invocation.verbose,
        invocation.log_level,
        invocation.color,
    );
    tracing::info!(
        command = invocation.command.name(),
        verbose = invocation.verbose,
        quiet = invocation.quiet,
        "dx invocation parsed"
    );
    if let Some(message) = dx_cli::platform::refusal(std::env::consts::OS, std::env::consts::ARCH) {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let mut err = io::stderr();
        let _ = writeln!(err, "dx: {message}");
        if invocation.output == OutputMode::Json {
            if let Ok(event) = error_event("unsupported_platform", &message, None, None, None) {
                if let Err(exit) = emit_event(&mut out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(
                &mut out,
                &command_finished(
                    operational_code(),
                    &FinishedCounts {
                        results_complete: Some(false),
                        ..FinishedCounts::default()
                    },
                ),
            ) {
                return exit;
            }
        }
        if let Err(exit) = flush_out(&mut out) {
            return exit;
        }
        return operational_code();
    }
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            let _ = writeln!(io::stderr(), "dx: cannot read working directory: {error}");
            return pre_exec_code();
        }
    };
    let start = dx_process::workspace_start(&cwd);
    let workspace = match discover_real(&start, invocation.workspace.as_deref().map(Path::new)) {
        Ok(workspace) => workspace,
        Err(error) => {
            if is_discovery_exempt(invocation.command) {
                invocation
                    .workspace
                    .as_deref()
                    .map(Path::new)
                    .map(|raw| {
                        let display = dx_process::resolve_override_display(raw, &start);
                        dx_process::canonicalize_or_keep(&display)
                    })
                    .unwrap_or(start)
            } else {
                let _ = writeln!(io::stderr(), "dx: cannot resolve workspace: {error}");
                return pre_exec_code();
            }
        }
    };
    if invocation.here {
        match dx_cli::args::apply_here(&invocation, &workspace, &cwd) {
            Ok(resolved) => invocation = resolved,
            Err(detail) => {
                let _ = writeln!(io::stderr(), "dx: {detail}");
                return pre_exec_code();
            }
        }
    }
    let pin = dx_cli::skew::read_pin(&workspace);
    match dx_cli::skew::disposition(&invocation, dx_cli::skew::is_skewed(&pin)) {
        dx_cli::skew::SkewDisposition::Proceed => {}
        dx_cli::skew::SkewDisposition::Warn => {
            let _ = writeln!(
                io::stderr(),
                "dx: warning: {} (proceeding: read-only or --dry-run invocation)",
                dx_cli::skew::diagnostic(&pin)
            );
        }
        dx_cli::skew::SkewDisposition::Refuse => {
            let message = dx_cli::skew::diagnostic(&pin);
            let stdout = io::stdout();
            let mut out = stdout.lock();
            let mut err = io::stderr();
            let _ = writeln!(err, "dx: {message}");
            if invocation.output == OutputMode::Json {
                if let Ok(event) = error_event("version_skew", &message, None, None, None) {
                    if let Err(exit) = emit_event(&mut out, &event) {
                        return exit;
                    }
                }
                if let Err(exit) = emit_event(
                    &mut out,
                    &command_finished(
                        operational_code(),
                        &FinishedCounts {
                            results_complete: Some(false),
                            ..FinishedCounts::default()
                        },
                    ),
                ) {
                    return exit;
                }
            }
            if let Err(exit) = flush_out(&mut out) {
                return exit;
            }
            return operational_code();
        }
    }
    let pid = std::process::id();
    let (temp_dir, nonce) = match create_run_temp_dir(&std::env::temp_dir()) {
        Ok(run) => run,
        Err(error) => {
            let _ = writeln!(
                io::stderr(),
                "dx: cannot create temporary directory: {error}"
            );
            return pre_exec_code();
        }
    };
    let inherit_stdout = matches!(invocation.output, OutputMode::Text { .. })
        && !invocation
            .reports
            .iter()
            .any(|report| report.destination == "-");
    let runner = BinaryRunner { inherit_stdout };
    let query_runner = ProcessQueryRunner;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut err = io::stderr();
    let code = execute(
        &invocation,
        Env {
            workspace: &workspace,
            runner: &runner,
            query_runner: &query_runner,
            temp_dir: temp_dir.path(),
            pid,
            nonce,
            out: &mut out,
            err: &mut err,
            ci: dx_process::is_ci(),
        },
    );
    if let Err(exit) = flush_out(&mut out) {
        let temp_display = temp_dir.path().display().to_string();
        if let Err(error) = temp_dir.close() {
            let _ = writeln!(
                io::stderr(),
                "dx: warning: cannot remove temporary directory {temp_display}: {error}",
            );
        }
        return exit;
    }
    let temp_display = temp_dir.path().display().to_string();
    if let Err(error) = temp_dir.close() {
        let _ = writeln!(
            io::stderr(),
            "dx: warning: cannot remove temporary directory {temp_display}: {error}",
        );
    }
    code
}
