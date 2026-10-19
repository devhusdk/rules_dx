#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use dx_cli::args::{
    early_workspace_flag, freeze_workspace, is_discovery_exempt, is_help_request,
    json_output_intent, parse_with_ci, select_startup_defaults, DX_WORKSPACE_ENV,
};
use dx_cli::exec::common::flush_out;
use dx_cli::plan::create_run_temp_dir;
use dx_cli::{execute, Env, ProcessQueryRunner};
use dx_output::OutputMode;
use dx_process::lifecycle::{self, EnvPolicy, Exit, StreamMode, StreamOutcome, STREAM_DRAIN_WAIT};
use dx_process::{
    discover_real, operational_code, pre_exec_code, stdout_io_code, ChildStatus, Runner,
};

#[cfg(unix)]
extern "C" fn forward_to_child(signo: libc::c_int) {
    lifecycle::interrupt(signo);
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

fn reraise(exit: &Exit) {
    #[cfg(unix)]
    if let Exit::Signal(signo) = exit {
        unsafe {
            libc::signal(*signo, libc::SIG_DFL);
            libc::raise(*signo);
        }
    }
    #[cfg(not(unix))]
    let _ = exit;
}

fn spawn_streamed(
    argv: &[String],
    cwd: &Path,
    env: &[(&str, &str)],
    clear_env: bool,
    inherit_stdout: bool,
) -> io::Result<ChildStatus> {
    let variables: Vec<(OsString, OsString)> = env
        .iter()
        .map(|(key, value)| (OsString::from(*key), OsString::from(*value)))
        .collect();
    let spec = lifecycle::StreamSpec {
        argv: argv.iter().map(OsString::from).collect(),
        cwd: cwd.to_path_buf(),
        env: if clear_env {
            EnvPolicy::Controlled { vars: variables }
        } else {
            EnvPolicy::Inherited { extra: variables }
        },
        streams: if inherit_stdout {
            StreamMode::Inherited
        } else {
            StreamMode::StdoutToStderr
        },
        drain: STREAM_DRAIN_WAIT,
    };
    let code = match lifecycle::run_streamed(&spec)? {
        StreamOutcome::Finished { exit } => {
            reraise(&exit);
            exit.code()
        }
        StreamOutcome::Cancelled { exit } => {
            if let Some(exit) = &exit {
                reraise(exit);
            }
            None
        }
        StreamOutcome::Undrained { exit } => {
            reraise(&exit);
            None
        }
    };
    Ok(ChildStatus { code })
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

fn startup_failure(stderr_text: &str, code: &str, message: &str, exit: i32, json: bool) -> i32 {
    let _ = writeln!(io::stderr(), "{stderr_text}");
    if !json {
        return exit;
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if let Err(io_exit) = dx_cli::exec::common::emit_startup_outcome(&mut out, code, message, exit)
    {
        return io_exit;
    }
    exit
}

fn usage_error(message: &str, json: bool) -> i32 {
    startup_failure(
        &format!("dx: {message}\n{}", dx_cli::args::help::usage_banner()),
        "invalid_arguments",
        message,
        pre_exec_code(),
        json,
    )
}

fn main() {
    install_forwarding();
    let code = run();
    #[cfg(unix)]
    if let Some(signo) = lifecycle::interrupted() {
        unsafe {
            libc::signal(signo, libc::SIG_DFL);
            libc::raise(signo);
        }
    }
    std::process::exit(code);
}

fn run() -> i32 {
    // LCOV_EXCL_START - reason: thin run shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let env_get = |name: &str| std::env::var(name).ok();
    let is_ci = dx_process::is_ci();
    let bare_intent = json_output_intent(
        &args,
        &env_get,
        &dx_cli::args::FileDefaults::default(),
        is_ci,
    );
    match dx_cli::args::try_complete() {
        Ok(true) => return 0,
        Ok(false) => {}
        Err(message) => {
            return startup_failure(
                &format!("dx: {message}\n{}", dx_cli::args::help::usage_banner()),
                "completion_failed",
                &message,
                pre_exec_code(),
                bare_intent,
            );
        }
    }
    let initial_cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let initial_start = dx_process::workspace_start(&initial_cwd);
    let flag_workspace = early_workspace_flag(&args);
    let env_workspace = std::env::var(DX_WORKSPACE_ENV)
        .ok()
        .filter(|value| !value.is_empty());
    let (mut file_defaults, file_redirect) =
        match select_startup_defaults(&initial_start, flag_workspace, env_workspace) {
            Ok(selected) => (selected.defaults, selected.file_workspace),
            Err(detail) => {
                if is_help_request(&args) {
                    (dx_cli::args::FileDefaults::default(), None)
                } else {
                    return startup_failure(
                        &format!("dx: {detail}\n{}", dx_cli::args::help::usage_banner()),
                        "invalid_defaults",
                        &detail,
                        pre_exec_code(),
                        bare_intent,
                    );
                }
            }
        };
    if let Some(target) = file_redirect {
        let dir = dx_process::resolve_override_display(Path::new(&target), &initial_start);
        match select_startup_defaults(&dir, Some(target.clone()), None) {
            Ok(selected) => {
                file_defaults = freeze_workspace(&selected.defaults, &target);
            }
            Err(detail) => {
                if !is_help_request(&args) {
                    let json = json_output_intent(&args, &env_get, &file_defaults, is_ci);
                    return startup_failure(
                        &format!("dx: {detail}\n{}", dx_cli::args::help::usage_banner()),
                        "invalid_defaults",
                        &detail,
                        pre_exec_code(),
                        json,
                    );
                }
            }
        }
    };
    let mut invocation = match parse_with_ci(&args, &env_get, &file_defaults, is_ci) {
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
        Err(error) => {
            let json = json_output_intent(&args, &env_get, &file_defaults, is_ci);
            return usage_error(&error.to_string(), json);
        }
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
    if let Some(message) = dx_cli::platform::host_refusal() {
        return startup_failure(
            &format!("dx: {message}"),
            "unsupported_platform",
            &message,
            operational_code(),
            invocation.output == OutputMode::Json,
        );
    }
    let cwd = match std::env::current_dir() {
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
    if invocation.here {
        match dx_cli::args::apply_here(&invocation, &workspace, &cwd) {
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
    let (temp_dir, nonce) = match create_run_temp_dir(&std::env::temp_dir()) {
        Ok(run) => run,
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
