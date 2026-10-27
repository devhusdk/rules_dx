#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! A deterministic child process for tests that must not need a host shell.

use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

/// The usage text every run of the probe documents.
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
  --descendant-gate=PATH        end the descendant's sleep early once PATH exists
  --raise-signal=N              die by signal N (unix only)
  --count-sigint=PATH           append one byte to PATH per SIGINT instead of dying (unix only)
  --require-foreground          exit 3 unless this probe's group holds the terminal (unix only)
  --help                        write this text and exit 0

With PROCESS_PROBE_SHIM_LOG set, the probe ignores its arguments, writes the
working directory and then each argument to that file one per line, writes
PROCESS_PROBE_SHIM_STDOUT to stdout, and exits with PROCESS_PROBE_SHIM_EXIT.
";

/// The file a shim run records its working directory and arguments into.
const SHIM_LOG: &str = "PROCESS_PROBE_SHIM_LOG";
/// The text a shim run writes to stdout.
const SHIM_STDOUT: &str = "PROCESS_PROBE_SHIM_STDOUT";
/// The exit status a shim run reports.
const SHIM_EXIT: &str = "PROCESS_PROBE_SHIM_EXIT";

/// A malformed invocation.
const EXIT_USAGE: i32 = 64;
/// Output the child could not write.
const EXIT_OUTPUT: i32 = 74;
/// A required environment variable was unset or held another value.
const EXIT_ENV_MISSING: i32 = 70;
/// A forbidden environment variable was set.
const EXIT_ENV_SET: i32 = 71;
/// The probe's group did not hold the terminal.
const EXIT_BACKGROUND: i32 = 3;

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
    descendant_gate: Option<String>,
    raise_signal: Option<i32>,
    count_sigint: Option<String>,
    require_foreground: bool,
}

fn count<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T, String> {
    value
        .parse::<T>()
        .map_err(|_| format!("{flag} needs a number, got {value}"))
}

/// The flags that carry a value after `=`.
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
    "--descendant-gate",
    "--raise-signal",
    "--count-sigint",
];

/// The flags that stand alone.
const SWITCH_FLAGS: &[&str] = &[
    "--print-env-all",
    "--print-argv",
    "--print-cwd",
    "--print-exe",
    "--spawn-descendant",
    "--detached-descendant",
    "--descendant-own-group",
    "--require-foreground",
];

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
            "--exit-code" => {
                let code: i32 = count(value, flag)?;
                if !(0..=255).contains(&code) {
                    return Err(format!("{flag} needs 0-255, got {value}"));
                }
                plan.exit_code = code;
            }
            "--stdout-bytes" => plan.stdout_bytes = count(value, flag)?,
            "--stderr-bytes" => plan.stderr_bytes = count(value, flag)?,
            "--stdout-text" => plan.stdout_text = value.as_bytes().to_vec(),
            "--stderr-text" => plan.stderr_text = value.as_bytes().to_vec(),
            "--sleep-ms" => plan.sleep = Duration::from_millis(count(value, flag)?),
            "--gate" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.gate = Some(value.to_owned());
            }
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
            "--forbid-env" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a name"));
                }
                plan.forbidden.push(value.to_owned());
            }
            "--print-env" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a name"));
                }
                plan.print_env.push(value.to_owned());
            }
            "--descendant-exit" => {
                let code: i32 = count(value, flag)?;
                if !(0..=255).contains(&code) {
                    return Err(format!("{flag} needs 0-255, got {value}"));
                }
                plan.descendant_exit = code;
            }
            "--descendant-sleep-ms" => plan.descendant_sleep_ms = count(value, flag)?,
            "--marker" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.marker = Some(value.to_owned());
            }
            "--descendant-marker" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.descendant_marker = Some(value.to_owned());
            }
            "--alive-lock" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.alive_lock = Some(value.to_owned());
            }
            "--descendant-alive-lock" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.descendant_alive_lock = Some(value.to_owned());
            }
            "--descendant-gate" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.descendant_gate = Some(value.to_owned());
            }
            "--raise-signal" => plan.raise_signal = Some(count(value, flag)?),
            "--count-sigint" => {
                if value.is_empty() {
                    return Err(format!("{flag} needs a path"));
                }
                plan.count_sigint = Some(value.to_owned());
            }
            _ => plan.descendant_stdout_bytes = count(value, flag)?,
        }
    }
    Ok(plan)
}

fn write_zeros(out: &mut impl Write, count: usize) -> io::Result<()> {
    let chunk = vec![0u8; count.min(64 * 1024)];
    let mut left = count;
    while left > 0 {
        let take = left.min(chunk.len());
        out.write_all(&chunk[..take])?;
        left -= take;
    }
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
    let mut command = Command::new(exe);
    command.args(&args).stdin(Stdio::null());
    if plan.descendant_own_group {
        command.stdout(Stdio::null()).stderr(Stdio::null());
        own_group(&mut command);
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

fn start_descendant(plan: &Plan) -> i32 {
    match run_descendant(plan) {
        Ok(code) => code,
        Err(message) => fail(EXIT_OUTPUT, &message),
    }
}

#[cfg(unix)]
fn own_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn own_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x8;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
    const HANDLE_FLAG_INHERIT: u32 = 0x1;
    const STD_HANDLES: [u32; 3] = [0xFFFF_FFF6, 0xFFFF_FFF5, 0xFFFF_FFF4];
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
        fn SetHandleInformation(handle: *mut std::ffi::c_void, mask: u32, flags: u32) -> i32;
    }
    for which in STD_HANDLES {
        unsafe {
            let handle = GetStdHandle(which);
            if !handle.is_null() && handle as isize != -1 {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn own_group(_command: &mut Command) {}

#[cfg(unix)]
static SIGINT_LOG: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

#[cfg(unix)]
extern "C" fn count_sigint(_signo: i32) {
    unsafe extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    let fd = SIGINT_LOG.load(std::sync::atomic::Ordering::SeqCst);
    unsafe {
        write(fd, b"I".as_ptr(), 1);
    }
}

#[cfg(unix)]
fn start_counting_sigint(path: &str) -> io::Result<()> {
    use std::os::fd::IntoRawFd;
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
    unsafe {
        signal(SIGINT, count_sigint as *const () as usize);
    }
    std::fs::rename(&staging, path)
}

#[cfg(unix)]
fn holds_terminal() -> bool {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn tcgetpgrp(fd: i32) -> i32;
        fn getpgrp() -> i32;
    }
    let Ok(tty) = std::fs::File::open("/dev/tty") else {
        return false;
    };
    let foreground = unsafe { tcgetpgrp(tty.as_raw_fd()) };
    foreground > 0 && foreground == unsafe { getpgrp() }
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

fn fail(code: i32, message: &str) -> ! {
    let mut err = io::stderr();
    let _ = writeln!(err, "process_probe: {message}");
    std::process::exit(code);
}

fn report_output_error(error: io::Error) -> ! {
    fail(EXIT_OUTPUT, &format!("cannot write the report: {error}"));
}

/// The working directory without a Windows verbatim prefix.
fn plain_cwd() -> io::Result<String> {
    let cwd = env::current_dir()?.to_string_lossy().into_owned();
    if let Some(rest) = cwd.strip_prefix(r"\\?\") {
        return Ok(rest.to_owned());
    }
    Ok(cwd)
}

fn run_shim(log: &std::ffi::OsStr) -> ! {
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
        out.write_all(env_value_or_empty(SHIM_STDOUT).as_bytes())?;
        out.flush()
    });
    if let Err(error) = written {
        fail(EXIT_OUTPUT, &format!("cannot record the shim run: {error}"));
    }
    let code = env_value_or_empty(SHIM_EXIT);
    if code.is_empty() {
        std::process::exit(0);
    }
    match count::<i32>(&code, SHIM_EXIT) {
        Ok(code) if (0..=255).contains(&code) => std::process::exit(code),
        _ => fail(EXIT_USAGE, &format!("{SHIM_EXIT} needs 0-255, got {code}")),
    }
}

fn env_value_or_empty(name: &str) -> String {
    env::var_os(name)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn main() {
    if let Some(log) = env::var_os(SHIM_LOG).filter(|log| !log.is_empty()) {
        run_shim(&log);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        return;
    }
    let plan = match parse(&args) {
        Ok(plan) => plan,
        Err(message) => {
            let mut err = io::stderr();
            let _ = writeln!(err, "process_probe: {message}");
            let _ = err.write_all(USAGE.as_bytes());
            std::process::exit(EXIT_USAGE);
        }
    };
    for name in &plan.forbidden {
        if env::var_os(name).is_some() {
            fail(EXIT_ENV_SET, &format!("{name} is set"));
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
            fail(
                EXIT_ENV_MISSING,
                &format!("{name} must hold {wanted}, holds {}", env_value(name)),
            );
        }
    }
    let _alive = match plan.alive_lock.as_deref().map(hold_alive_lock) {
        Some(Err(error)) => fail(EXIT_OUTPUT, &format!("cannot hold the alive lock: {error}")),
        held => held,
    };
    #[cfg(unix)]
    if plan.require_foreground && !holds_terminal() {
        fail(
            EXIT_BACKGROUND,
            "this probe's group does not hold the terminal",
        );
    }
    #[cfg(not(unix))]
    if plan.require_foreground {
        fail(EXIT_USAGE, "--require-foreground needs unix");
    }
    #[cfg(unix)]
    if let Some(path) = &plan.count_sigint {
        if let Err(error) = start_counting_sigint(path) {
            fail(EXIT_OUTPUT, &format!("cannot count SIGINT: {error}"));
        }
    }
    #[cfg(not(unix))]
    if plan.count_sigint.is_some() {
        fail(EXIT_USAGE, "--count-sigint needs unix");
    }
    #[cfg(unix)]
    if let Some(signo) = plan.raise_signal {
        unsafe extern "C" {
            fn raise(sig: i32) -> i32;
        }
        unsafe {
            raise(signo);
        }
    }
    #[cfg(not(unix))]
    if plan.raise_signal.is_some() {
        fail(EXIT_USAGE, "--raise-signal needs unix");
    }
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
        report_output_error(error);
    }
    if let Err(error) = err
        .write_all(&plan.stderr_text)
        .and_then(|()| write_zeros(&mut err, plan.stderr_bytes))
        .and_then(|()| err.flush())
    {
        report_output_error(error);
    }
    drop(out);
    drop(err);
    let mut exit_code = plan.exit_code;
    if plan.detached_descendant {
        exit_code = start_descendant(&plan);
    }
    if !plan.sleep.is_zero() {
        sleep_until_gate(&plan);
    }
    if plan.spawn_descendant && !plan.detached_descendant {
        exit_code = start_descendant(&plan);
    }
    if let Some(marker) = &plan.marker {
        if let Err(error) = std::fs::write(marker, b"finished\n") {
            fail(EXIT_OUTPUT, &format!("cannot write the marker: {error}"));
        }
    }
    std::process::exit(exit_code);
}
