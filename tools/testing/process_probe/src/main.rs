#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::collections::BTreeMap;
use std::env;
use std::io::{self, Read, Write};
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

const USAGE: &str = "\
process_probe: a deterministic child for process tests.

Usage: process_probe [options]

  --exit-code=N                 exit with N (0-255)
  --stdout-bytes=N              write N zero bytes to stdout after any text
  --stderr-bytes=N              write N zero bytes to stderr after any text
  --stdout-text=TEXT            write TEXT to stdout
  --stderr-text=TEXT            write TEXT to stderr
  --sleep-ms=N                  sleep N milliseconds before exiting
  --gate=PATH                   end the sleep early once PATH exists
  --require-env=NAME[=VALUE]    exit 70 unless NAME holds VALUE
  --forbid-env=NAME             exit 71 unless NAME is unset
  --print-env=NAME              write NAME's value or <unset> to stdout
  --print-env-all               write every variable to stdout as KEY=VALUE lines
  --print-argv                  write each argument to stdout, one per line
  --print-cwd                   write the working directory to stdout
  --print-exe                   write this probe's executable path to stdout
  --spawn-descendant            run a descendant probe and exit with its status
  --detached-descendant         start the descendant before sleeping; do not wait
  --descendant-own-group        detach the descendant like a daemon, with no output
  --descendant-exit=N           exit code the descendant reports
  --descendant-sleep-ms=N       milliseconds the descendant sleeps
  --descendant-stdout-bytes=N   zero bytes the descendant writes to stdout
  --descendant-marker=PATH      file the descendant creates when it finishes
  --marker=PATH                 file this probe creates when it finishes
  --alive-lock=PATH             hold a lock on PATH until this probe exits
  --descendant-alive-lock=PATH  lock the descendant holds; wait until it is held
  --nested-server-lock=PATH     descendant is a client that starts a detached server
                                holding PATH; wait until it is held
  --descendant-gate=PATH        end the descendant's sleep early once PATH exists
  --raise-signal=N              die by signal N (unix only)
  --descendant-count-sigint=PATH  the descendant appends one byte to PATH per SIGINT (unix only)
  --count-sigint=PATH           append one byte to PATH per SIGINT instead of dying (unix only)
  --ctrl-c-console=PID          send Ctrl-C to the console of process PID, then exit (windows only)
  --ignore-console-ctrl         ignore console Ctrl-C in this probe and its descendant (windows only)
  --require-foreground          exit 3 unless this probe's group holds the terminal (unix only)
  --help                        write this text and exit 0

With PROCESS_PROBE_BAZEL_DIR set, the probe acts as a fake bazel: for build and
run it writes start-N (its arguments) in that directory; run also keeps an app
child alive holding alive-N until the stop file appears, and waits for it.

With PROCESS_PROBE_CALL_LOG set, the probe ignores its arguments, writes the
working directory and then each argument to that file one per line, writes
PROCESS_PROBE_CALL_STDOUT to stdout and PROCESS_PROBE_CALL_STDERR to stderr, and
exits with PROCESS_PROBE_CALL_EXIT.
";

const CALL_LOG: &str = "PROCESS_PROBE_CALL_LOG";
const CALL_STDOUT: &str = "PROCESS_PROBE_CALL_STDOUT";
const CALL_STDERR: &str = "PROCESS_PROBE_CALL_STDERR";
const CALL_EXIT: &str = "PROCESS_PROBE_CALL_EXIT";
const BAZEL_DIR: &str = "PROCESS_PROBE_BAZEL_DIR";

const APP_ARGS: &str = "PROCESS_PROBE_APP_ARGS";

const APP_DESCENDANT: &str = "PROCESS_PROBE_APP_DESCENDANT";

const EXIT_USAGE: i32 = 64;
const EXIT_OUTPUT: i32 = 74;
const EXIT_ENV_MISSING: i32 = 70;
const EXIT_ENV_SET: i32 = 71;
#[cfg(unix)]
const EXIT_BACKGROUND: i32 = 3;

#[expect(
    clippy::struct_excessive_bools,
    reason = "one field per command-line switch"
)]
#[derive(Debug, Default)]
struct Plan {
    exit_code: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    stdout_text: Vec<u8>,
    stderr_text: Vec<u8>,
    sleep: Duration,
    gate: Option<String>,
    required: Vec<(String, Option<String>)>,
    forbidden: Vec<String>,
    print_env: Vec<String>,
    print_env_all: bool,
    print_argv: bool,
    print_cwd: bool,
    print_exe: bool,
    spawn_descendant: bool,
    detached_descendant: bool,
    descendant_own_group: bool,
    descendant_exit: i32,
    descendant_sleep_ms: u64,
    descendant_stdout_bytes: usize,
    descendant_marker: Option<String>,
    marker: Option<String>,
    alive_lock: Option<String>,
    descendant_alive_lock: Option<String>,
    nested_server_lock: Option<String>,
    descendant_gate: Option<String>,
    raise_signal: Option<i32>,
    count_sigint: Option<String>,
    descendant_count_sigint: Option<String>,
    require_foreground: bool,
    ctrl_c_console: Option<u32>,
    ignore_console_ctrl: bool,
}

fn count<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    value
        .parse::<T>()
        .map_err(|error| format!("{flag} needs a number, got {value}: {error}"))
}

const VALUE_FLAGS: &[&str] = &[
    "--exit-code",
    "--stdout-bytes",
    "--stderr-bytes",
    "--stdout-text",
    "--stderr-text",
    "--sleep-ms",
    "--gate",
    "--require-env",
    "--forbid-env",
    "--print-env",
    "--descendant-exit",
    "--descendant-sleep-ms",
    "--descendant-stdout-bytes",
    "--descendant-marker",
    "--marker",
    "--alive-lock",
    "--descendant-alive-lock",
    "--nested-server-lock",
    "--descendant-gate",
    "--raise-signal",
    "--count-sigint",
    "--descendant-count-sigint",
    "--ctrl-c-console",
];

const SWITCH_FLAGS: &[&str] = &[
    "--print-env-all",
    "--print-argv",
    "--print-cwd",
    "--print-exe",
    "--spawn-descendant",
    "--detached-descendant",
    "--descendant-own-group",
    "--require-foreground",
    "--ignore-console-ctrl",
];

fn non_empty(value: &str, flag: &str, what: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err(format!("{flag} needs {what}"));
    }
    Ok(value.to_owned())
}

fn exit_status(value: &str, flag: &str) -> Result<i32, String> {
    let code: i32 = count(value, flag)?;
    if !(0..=255).contains(&code) {
        return Err(format!("{flag} needs 0-255, got {value}"));
    }
    Ok(code)
}

fn parse(args: &[String]) -> Result<Plan, String> {
    let mut plan = Plan::default();
    for arg in args {
        let (flag, value) = arg.split_once('=').unwrap_or((arg.as_str(), ""));
        if SWITCH_FLAGS.contains(&flag) {
            if arg.contains('=') {
                return Err(format!("{flag} takes no value"));
            }
            match flag {
                "--print-env-all" => plan.print_env_all = true,
                "--print-argv" => plan.print_argv = true,
                "--print-cwd" => plan.print_cwd = true,
                "--print-exe" => plan.print_exe = true,
                "--require-foreground" => plan.require_foreground = true,
                "--ignore-console-ctrl" => plan.ignore_console_ctrl = true,
                "--detached-descendant" => {
                    plan.detached_descendant = true;
                    plan.spawn_descendant = true;
                }
                "--descendant-own-group" => plan.descendant_own_group = true,
                _ => plan.spawn_descendant = true,
            }
            continue;
        }
        if !VALUE_FLAGS.contains(&flag) {
            return Err(format!("unknown flag {flag}"));
        }
        if !arg.contains('=') {
            return Err(format!("{flag} needs a value"));
        }
        match flag {
            "--exit-code" => plan.exit_code = exit_status(value, flag)?,
            "--stdout-bytes" => plan.stdout_bytes = count(value, flag)?,
            "--stderr-bytes" => plan.stderr_bytes = count(value, flag)?,
            "--stdout-text" => plan.stdout_text = value.as_bytes().to_vec(),
            "--stderr-text" => plan.stderr_text = value.as_bytes().to_vec(),
            "--sleep-ms" => plan.sleep = Duration::from_millis(count(value, flag)?),
            "--gate" => plan.gate = Some(non_empty(value, flag, "a path")?),
            "--require-env" => {
                let (name, expected) = match value.split_once('=') {
                    Some((name, expected)) => (name, Some(expected.to_owned())),
                    None => (value, None),
                };
                if name.is_empty() {
                    return Err(format!("{flag} needs a name"));
                }
                plan.required.push((name.to_owned(), expected));
            }
            "--forbid-env" => plan.forbidden.push(non_empty(value, flag, "a name")?),
            "--print-env" => plan.print_env.push(non_empty(value, flag, "a name")?),
            "--descendant-exit" => plan.descendant_exit = exit_status(value, flag)?,
            "--descendant-sleep-ms" => plan.descendant_sleep_ms = count(value, flag)?,
            "--marker" => plan.marker = Some(non_empty(value, flag, "a path")?),
            "--descendant-marker" => {
                plan.descendant_marker = Some(non_empty(value, flag, "a path")?)
            }
            "--alive-lock" => plan.alive_lock = Some(non_empty(value, flag, "a path")?),
            "--descendant-alive-lock" => {
                plan.descendant_alive_lock = Some(non_empty(value, flag, "a path")?)
            }
            "--nested-server-lock" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.nested_server_lock = Some(value.to_owned());
                plan.spawn_descendant = true;
                plan.detached_descendant = true;
            }
            "--descendant-gate" => plan.descendant_gate = Some(non_empty(value, flag, "a path")?),
            "--ctrl-c-console" => plan.ctrl_c_console = Some(count(value, flag)?),
            "--raise-signal" => plan.raise_signal = Some(count(value, flag)?),
            "--descendant-count-sigint" => {
                plan.descendant_count_sigint = Some(non_empty(value, flag, "a path")?)
            }
            "--count-sigint" => plan.count_sigint = Some(non_empty(value, flag, "a path")?),
            _ => plan.descendant_stdout_bytes = count(value, flag)?,
        }
    }
    Ok(plan)
}

fn write_zeros(out: &mut impl Write, count: usize) -> io::Result<()> {
    io::copy(&mut io::repeat(0).take(count as u64), out)?;
    Ok(())
}

fn env_value(name: &str) -> String {
    match env::var_os(name) {
        Some(value) => value.to_string_lossy().into_owned(),
        None => "<unset>".to_owned(),
    }
}

fn write_report(plan: &Plan, out: &mut impl Write) -> io::Result<()> {
    for name in &plan.print_env {
        writeln!(out, "{}", env_value(name))?;
    }
    if plan.print_env_all {
        let mut vars: BTreeMap<String, String> = BTreeMap::new();
        for (key, value) in env::vars_os() {
            vars.insert(
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            );
        }
        for (key, value) in vars {
            writeln!(out, "{key}={value}")?;
        }
    }
    if plan.print_argv {
        for arg in env::args().skip(1) {
            writeln!(out, "{arg}")?;
        }
    }
    if plan.print_cwd {
        let cwd = env::current_dir().map_err(|err| io::Error::other(err.to_string()))?;
        writeln!(out, "{}", cwd.to_string_lossy())?;
    }
    if plan.print_exe {
        let exe = env::current_exe().map_err(|err| io::Error::other(err.to_string()))?;
        writeln!(out, "{}", exe.to_string_lossy())?;
    }
    Ok(())
}

fn run_descendant(plan: &Plan) -> Result<i32, String> {
    let exe = env::current_exe().map_err(|err| format!("cannot name the probe: {err}"))?;
    let mut args = vec![
        format!("--exit-code={}", plan.descendant_exit),
        format!("--sleep-ms={}", plan.descendant_sleep_ms),
        format!("--stdout-bytes={}", plan.descendant_stdout_bytes),
    ];
    if let Some(marker) = &plan.descendant_marker {
        args.push(format!("--marker={marker}"));
    }
    if let Some(lock) = &plan.descendant_alive_lock {
        args.push(format!("--alive-lock={lock}"));
    }
    if let Some(gate) = &plan.descendant_gate {
        args.push(format!("--gate={gate}"));
    }
    if let Some(log) = &plan.descendant_count_sigint {
        args.push(format!("--count-sigint={log}"));
    }
    if plan.ignore_console_ctrl {
        args.push("--ignore-console-ctrl".to_owned());
    }
    if let Some(lock) = &plan.nested_server_lock {
        args = vec![
            "--detached-descendant".to_owned(),
            "--descendant-own-group".to_owned(),
            "--descendant-sleep-ms=30000".to_owned(),
            format!("--descendant-alive-lock={lock}"),
            "--sleep-ms=30000".to_owned(),
        ];
    }
    let mut command = Command::new(exe);
    command.args(&args).stdin(Stdio::null());
    if let Some(lock) = &plan.nested_server_lock {
        let _client = command
            .spawn()
            .map_err(|err| format!("cannot spawn a client: {err}"))?;
        wait_for_lock(lock)?;
        return Ok(plan.exit_code);
    }
    if plan.descendant_own_group {
        command.stdout(Stdio::null()).stderr(Stdio::null());
        own_group(&mut command)?;
    }
    if let Some(lock) = &plan.descendant_alive_lock {
        let mut child = command
            .spawn()
            .map_err(|err| format!("cannot spawn a descendant: {err}"))?;
        wait_for_lock(lock)?;
        if plan.detached_descendant {
            return Ok(plan.exit_code);
        }
        let status = child
            .wait()
            .map_err(|err| format!("cannot wait for the descendant: {err}"))?;
        return Ok(status.code().unwrap_or(EXIT_USAGE));
    }
    if plan.detached_descendant {
        return command
            .spawn()
            .map(|_| plan.exit_code)
            .map_err(|err| format!("cannot spawn a descendant: {err}"));
    }
    let status = command
        .status()
        .map_err(|err| format!("cannot spawn a descendant: {err}"))?;
    Ok(status.code().unwrap_or(EXIT_USAGE))
}

fn wait_for_lock(path: &str) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !std::path::Path::new(path).exists() {
        if std::time::Instant::now() >= deadline {
            return Err(format!("the descendant never took {path}"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn hold_alive_lock(path: &str) -> io::Result<std::fs::File> {
    let staging = format!("{path}.staging");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&staging)?;
    file.try_lock().map_err(io::Error::other)?;
    std::fs::rename(&staging, path)?;
    Ok(file)
}

fn start_descendant(plan: &Plan) -> Result<i32, Failure> {
    run_descendant(plan).map_err(|message| fail(EXIT_OUTPUT, &message))
}

#[cfg(unix)]
fn own_group(command: &mut Command) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    Ok(())
}

#[cfg(windows)]
fn own_group(command: &mut Command) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x8;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
    dx_os_sys::win32::disinherit_std_handles().map_err(|error| {
        format!("cannot keep the standard handles from the descendant: {error}")
    })?;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn own_group(_command: &mut Command) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
static SIGINT_LOG: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

#[cfg(unix)]
#[expect(unsafe_code, reason = "a signal handler calls libc write")]
extern "C" fn count_sigint(_signo: i32) {
    // SAFETY: the declaration matches the libc signature of write.
    unsafe extern "C" {
        fn write(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize;
    }
    let fd = SIGINT_LOG.load(std::sync::atomic::Ordering::SeqCst);
    // SAFETY: fd is the open log descriptor stored at startup and the buffer is one byte.
    unsafe {
        write(fd, b"I".as_ptr().cast(), 1);
    }
}

#[cfg(unix)]
#[expect(unsafe_code, reason = "installing a handler needs libc signal")]
fn start_counting_sigint(path: &str) -> io::Result<()> {
    use std::os::fd::IntoRawFd;
    // SAFETY: the declaration matches the libc signature of signal.
    unsafe extern "C" {
        fn signal(signo: i32, handler: usize) -> usize;
    }
    const SIGINT: i32 = 2;
    let staging = format!("{path}.staging");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&staging)?;
    SIGINT_LOG.store(log.into_raw_fd(), std::sync::atomic::Ordering::SeqCst);
    // SAFETY: count_sigint is an extern C handler that only calls write.
    unsafe {
        signal(SIGINT, count_sigint as *const () as usize);
    }
    std::fs::rename(&staging, path)
}

#[cfg(unix)]
#[expect(
    unsafe_code,
    reason = "terminal process group queries have no safe std API"
)]
fn holds_terminal() -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: the declarations match the libc signatures of tcgetpgrp and getpgrp.
    unsafe extern "C" {
        fn tcgetpgrp(fd: i32) -> i32;
        fn getpgrp() -> i32;
    }
    let Ok(tty) = std::fs::File::open("/dev/tty") else {
        return false;
    };
    // SAFETY: tty is an open descriptor for the duration of the call.
    let foreground = unsafe { tcgetpgrp(tty.as_raw_fd()) };
    // SAFETY: getpgrp has no preconditions.
    foreground > 0 && foreground == unsafe { getpgrp() }
}

#[cfg(unix)]
#[expect(unsafe_code, reason = "raise has no safe std API")]
fn raise_signal(signo: i32) {
    // SAFETY: the declaration matches the libc signature of raise.
    unsafe extern "C" {
        fn raise(sig: i32) -> i32;
    }
    // SAFETY: raise takes a plain signal number and has no memory preconditions.
    unsafe {
        raise(signo);
    }
}

fn sleep_until_gate(plan: &Plan) {
    let Some(gate) = &plan.gate else {
        std::thread::sleep(plan.sleep);
        return;
    };
    let deadline = std::time::Instant::now() + plan.sleep;
    while !std::path::Path::new(gate).exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Failure {
    code: i32,
    message: String,
}

fn fail(code: i32, message: &str) -> Failure {
    Failure {
        code,
        message: message.to_owned(),
    }
}

fn report_output_error(error: &io::Error) -> Failure {
    fail(EXIT_OUTPUT, &format!("cannot write the report: {error}"))
}

fn plain_cwd() -> io::Result<String> {
    let cwd = dx_path::without_verbatim(&env::current_dir()?);
    Ok(cwd.to_string_lossy().into_owned())
}

fn run_logged_call(log: &std::ffi::OsStr) -> Result<i32, Failure> {
    let mut record = String::new();
    let written = plain_cwd().and_then(|cwd| {
        record.push_str(&cwd);
        record.push('\n');
        for arg in env::args_os().skip(1) {
            record.push_str(&arg.to_string_lossy());
            record.push('\n');
        }
        std::fs::write(log, record.as_bytes())?;
        let mut out = io::stdout().lock();
        out.write_all(env_value_or_empty(CALL_STDOUT).as_bytes())?;
        out.flush()?;
        let mut err = io::stderr().lock();
        err.write_all(env_value_or_empty(CALL_STDERR).as_bytes())?;
        err.flush()
    });
    if let Err(error) = written {
        return Err(fail(
            EXIT_OUTPUT,
            &format!("cannot record the logged call: {error}"),
        ));
    }
    let code = env_value_or_empty(CALL_EXIT);
    if code.is_empty() {
        return Ok(0);
    }
    match count::<i32>(&code, CALL_EXIT) {
        Ok(code) if (0..=255).contains(&code) => Ok(code),
        _ => Err(fail(
            EXIT_USAGE,
            &format!("{CALL_EXIT} needs 0-255, got {code}"),
        )),
    }
}

fn env_value_or_empty(name: &str) -> String {
    env::var_os(name)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn run_fake_bazel(dir: &std::path::Path) -> Result<i32, Failure> {
    let args: Vec<String> = env::args().skip(1).collect();
    let verb = args.iter().find(|arg| *arg == "build" || *arg == "run");
    let Some(verb) = verb else {
        return Ok(0);
    };
    fake_bazel_launch(dir, verb == "run", &args.join(" "))
        .map_err(|message| fail(EXIT_OUTPUT, &message))
}

fn fake_bazel_launch(dir: &std::path::Path, app: bool, args: &str) -> Result<i32, String> {
    let launches = std::fs::read_dir(dir)
        .map_err(|err| format!("cannot read {}: {err}", dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("start-"))
        .count();
    let n = launches + 1;
    std::fs::write(dir.join(format!("start-{n}")), args)
        .map_err(|err| format!("cannot record the launch: {err}"))?;
    if !app {
        return Ok(0);
    }
    let exe = env::current_exe().map_err(|err| format!("cannot name the probe: {err}"))?;
    let lock = dir.join(format!("alive-{n}"));
    let extra = env_value_or_empty(APP_ARGS);
    let mut command = Command::new(exe);
    command
        .env_remove(BAZEL_DIR)
        .env_remove(APP_ARGS)
        .env_remove(APP_DESCENDANT)
        .args(extra.split_whitespace());
    if !env_value_or_empty(APP_DESCENDANT).is_empty() {
        command
            .arg("--detached-descendant")
            .arg("--descendant-sleep-ms=120000")
            .arg(format!(
                "--descendant-alive-lock={}",
                dir.join(format!("child-{n}")).display()
            ));
    }
    let mut child = command
        .arg("--sleep-ms=120000")
        .arg(format!("--gate={}", dir.join("stop").display()))
        .arg(format!("--alive-lock={}", lock.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("cannot start the app: {err}"))?;
    let status = child
        .wait()
        .map_err(|err| format!("cannot wait for the app: {err}"))?;
    Ok(status.code().unwrap_or(EXIT_USAGE))
}

fn check_environment(plan: &Plan) -> Result<(), Failure> {
    for name in &plan.forbidden {
        if env::var_os(name).is_some() {
            return Err(fail(EXIT_ENV_SET, &format!("{name} is set")));
        }
    }
    for (name, expected) in &plan.required {
        let holds = match (env::var(name), expected) {
            (Ok(_), None) => true,
            (Ok(have), Some(want)) => have == *want,
            (Err(_), _) => false,
        };
        if !holds {
            let wanted = expected.clone().unwrap_or_else(|| "<any>".to_owned());
            return Err(fail(
                EXIT_ENV_MISSING,
                &format!("{name} must hold {wanted}, holds {}", env_value(name)),
            ));
        }
    }
    Ok(())
}

fn apply_signal_flags(plan: &Plan) -> Result<(), Failure> {
    #[cfg(unix)]
    if plan.require_foreground && !holds_terminal() {
        return Err(fail(
            EXIT_BACKGROUND,
            "this probe's group does not hold the terminal",
        ));
    }
    #[cfg(not(unix))]
    if plan.require_foreground {
        return Err(fail(EXIT_USAGE, "--require-foreground needs unix"));
    }
    #[cfg(unix)]
    if let Some(path) = &plan.count_sigint
        && let Err(error) = start_counting_sigint(path)
    {
        return Err(fail(EXIT_OUTPUT, &format!("cannot count SIGINT: {error}")));
    }
    #[cfg(not(unix))]
    if plan.count_sigint.is_some() {
        return Err(fail(EXIT_USAGE, "--count-sigint needs unix"));
    }
    #[cfg(unix)]
    if let Some(signo) = plan.raise_signal {
        raise_signal(signo);
    }
    #[cfg(not(unix))]
    if plan.raise_signal.is_some() {
        return Err(fail(EXIT_USAGE, "--raise-signal needs unix"));
    }
    Ok(())
}

fn main() -> ExitCode {
    let code = run().unwrap_or_else(|failure| {
        dx_output::write_stderr(
            &mut io::stderr(),
            format_args!("process_probe: {}\n", failure.message),
        );
        failure.code
    });
    u8::try_from(code).map_or(ExitCode::FAILURE, ExitCode::from)
}

fn run() -> Result<i32, Failure> {
    if let Some(dir) = env::var_os(BAZEL_DIR).filter(|dir| !dir.is_empty()) {
        return run_fake_bazel(std::path::Path::new(&dir));
    }
    if let Some(log) = env::var_os(CALL_LOG).filter(|log| !log.is_empty()) {
        return run_logged_call(&log);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        return Ok(0);
    }
    let plan = match parse(&args) {
        Ok(plan) => plan,
        Err(message) => {
            dx_output::write_stderr(
                &mut io::stderr(),
                format_args!("process_probe: {message}\n{USAGE}"),
            );
            return Ok(EXIT_USAGE);
        }
    };
    #[cfg(windows)]
    {
        if let Some(pid) = plan.ctrl_c_console {
            return dx_os_sys::console::send_ctrl_c(pid)
                .map(|()| 0)
                .map_err(|error| fail(EXIT_OUTPUT, &error.to_string()));
        }
        if plan.ignore_console_ctrl {
            dx_os_sys::console::ignore_ctrl_c().map_err(|error| {
                fail(
                    EXIT_OUTPUT,
                    &format!("cannot ignore console Ctrl-C: {error}"),
                )
            })?;
        }
    }
    #[cfg(not(windows))]
    if plan.ctrl_c_console.is_some() || plan.ignore_console_ctrl {
        return Err(fail(EXIT_USAGE, "the console Ctrl-C flags need windows"));
    }
    check_environment(&plan)?;
    let _alive = match plan.alive_lock.as_deref().map(hold_alive_lock) {
        Some(Err(error)) => {
            return Err(fail(
                EXIT_OUTPUT,
                &format!("cannot hold the alive lock: {error}"),
            ));
        }
        held => held,
    };
    apply_signal_flags(&plan)?;
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    if let Err(error) = out
        .write_all(&plan.stdout_text)
        .and_then(|()| write_zeros(&mut out, plan.stdout_bytes))
        .and_then(|()| write_report(&plan, &mut out))
        .and_then(|()| out.flush())
    {
        return Err(report_output_error(&error));
    }
    if let Err(error) = err
        .write_all(&plan.stderr_text)
        .and_then(|()| write_zeros(&mut err, plan.stderr_bytes))
        .and_then(|()| err.flush())
    {
        return Err(report_output_error(&error));
    }
    drop(out);
    drop(err);
    let mut exit_code = plan.exit_code;
    if plan.detached_descendant {
        exit_code = start_descendant(&plan)?;
    }
    if !plan.sleep.is_zero() {
        sleep_until_gate(&plan);
    }
    if plan.spawn_descendant && !plan.detached_descendant {
        exit_code = start_descendant(&plan)?;
    }
    if let Some(marker) = &plan.marker
        && let Err(error) = std::fs::write(marker, b"finished\n")
    {
        return Err(fail(
            EXIT_OUTPUT,
            &format!("cannot write the marker: {error}"),
        ));
    }
    Ok(exit_code)
}
