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
  --require-env=NAME[=VALUE]    exit 70 unless NAME holds VALUE
  --forbid-env=NAME             exit 71 unless NAME is unset
  --print-env=NAME              write NAME's value or <unset> to stdout
  --print-env-all               write every variable to stdout as KEY=VALUE lines
  --print-argv                  write each argument to stdout, one per line
  --print-cwd                   write the working directory to stdout
  --spawn-descendant            run a descendant probe and exit with its status
  --detached-descendant         start the descendant and exit without waiting
  --descendant-exit=N           exit code the descendant reports
  --descendant-sleep-ms=N       milliseconds the descendant sleeps
  --descendant-stdout-bytes=N   zero bytes the descendant writes to stdout
  --descendant-marker=PATH      file the descendant creates when it finishes
  --marker=PATH                 file this probe creates when it finishes
  --raise-signal=N              die by signal N (unix only)
  --help                        write this text and exit 0
";

/// A malformed invocation.
const EXIT_USAGE: i32 = 64;
/// Output the child could not write.
const EXIT_OUTPUT: i32 = 74;
/// A required environment variable was unset or held another value.
const EXIT_ENV_MISSING: i32 = 70;
/// A forbidden environment variable was set.
const EXIT_ENV_SET: i32 = 71;

#[derive(Debug, Default)]
struct Plan {
    exit_code: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    stdout_text: Vec<u8>,
    stderr_text: Vec<u8>,
    sleep: Duration,
    required: Vec<(String, Option<String>)>,
    forbidden: Vec<String>,
    print_env: Vec<String>,
    print_env_all: bool,
    print_argv: bool,
    print_cwd: bool,
    spawn_descendant: bool,
    detached_descendant: bool,
    descendant_exit: i32,
    descendant_sleep_ms: u64,
    descendant_stdout_bytes: usize,
    descendant_marker: Option<String>,
    marker: Option<String>,
    raise_signal: Option<i32>,
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
    "--require-env",
    "--forbid-env",
    "--print-env",
    "--descendant-exit",
    "--descendant-sleep-ms",
    "--descendant-stdout-bytes",
    "--descendant-marker",
    "--marker",
    "--raise-signal",
];

/// The flags that stand alone.
const SWITCH_FLAGS: &[&str] = &[
    "--print-env-all",
    "--print-argv",
    "--print-cwd",
    "--spawn-descendant",
    "--detached-descendant",
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
                "--detached-descendant" => {
                    plan.detached_descendant = true;
                    plan.spawn_descendant = true;
                }
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
            "--raise-signal" => plan.raise_signal = Some(count(value, flag)?),
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
    let mut command = Command::new(exe);
    command.args(&args).stdin(Stdio::null());
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

fn fail(code: i32, message: &str) -> ! {
    let mut err = io::stderr();
    let _ = writeln!(err, "process_probe: {message}");
    std::process::exit(code);
}

fn report_output_error(error: io::Error) -> ! {
    fail(EXIT_OUTPUT, &format!("cannot write the report: {error}"));
}

fn main() {
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
    if !plan.sleep.is_zero() {
        std::thread::sleep(plan.sleep);
    }
    let mut exit_code = plan.exit_code;
    if plan.spawn_descendant {
        exit_code = match run_descendant(&plan) {
            Ok(code) => code,
            Err(message) => fail(EXIT_OUTPUT, &message),
        };
    }
    if let Some(marker) = &plan.marker {
        if let Err(error) = std::fs::write(marker, b"finished\n") {
            fail(EXIT_OUTPUT, &format!("cannot write the marker: {error}"));
        }
    }
    std::process::exit(exit_code);
}
