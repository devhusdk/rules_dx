//! Runs the check-only replay of one compile action as `dx compile_check`.
//!
//! usage: `dx compile_check LOG STATUS [OPTION]... -- STEP [--dx-next-- STEP]...`
//!
//! - `--append IDX DEST --line L ...` copies argv[IDX] of the first step to DEST, adds one line
//!   per `--line`, then runs the step with DEST in its place.
//! - `--alias FROM TO` replaces FROM with TO (fixed strings) in the log.
//!
//! Steps run in order and stop at the first nonzero exit. Their stderr is collected in LOG and
//! the last exit code is written to STATUS.

#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs the compiler steps of one replayed action, which exit on their own"
)]

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

#[cfg(test)]
mod lib_tests;

const PREFIX: &str = "compile_check: ";
const NEXT: &str = "--dx-next--";
const USAGE: &str = "usage: LOG STATUS [OPTION]... -- STEP [--dx-next-- STEP]...";

/// The parsed command line of one replay.
#[derive(Debug, Default, PartialEq, Eq)]
struct Replay {
    log: String,
    status: String,
    append: Option<Append>,
    aliases: Vec<(String, String)>,
    steps: Vec<Vec<String>>,
}

/// Where the first step's flag file is copied, and the lines added to the copy.
#[derive(Debug, Default, PartialEq, Eq)]
struct Append {
    idx: usize,
    dest: String,
    lines: Vec<String>,
}

/// Runs one replay from its command line and returns the exit code of this process.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    let replay = match parse(args) {
        Ok(replay) => replay,
        Err(message) => {
            eprintln!("{PREFIX}{message}");
            return 2;
        }
    };
    match run(&replay) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{PREFIX}{message}");
            1
        }
    }
}

fn parse(args: &[String]) -> Result<Replay, String> {
    let (log, status, mut rest) = match args {
        [log, status, rest @ ..] if !rest.is_empty() => (log, status, rest),
        _ => return Err(USAGE.to_owned()),
    };
    let mut replay = Replay {
        log: log.clone(),
        status: status.clone(),
        ..Replay::default()
    };
    let mut lines = Vec::new();
    let mut append: Option<(usize, String)> = None;
    loop {
        match rest {
            [flag, idx, dest, tail @ ..] if flag == "--append" => {
                let idx = idx
                    .parse()
                    .map_err(|_| format!("--append index {idx:?} is not a number"))?;
                append = Some((idx, dest.clone()));
                rest = tail;
            }
            [flag, line, tail @ ..] if flag == "--line" => {
                lines.push(line.clone());
                rest = tail;
            }
            [flag, from, to, tail @ ..] if flag == "--alias" => {
                replay.aliases.push((from.clone(), to.clone()));
                rest = tail;
            }
            [flag, tail @ ..] if flag == "--" => {
                rest = tail;
                break;
            }
            [flag, ..] => return Err(format!("unknown option {flag}")),
            [] => return Err(USAGE.to_owned()),
        }
    }
    replay.append = append.map(|(idx, dest)| Append { idx, dest, lines });
    replay.steps = rest
        .split(|arg| arg == NEXT)
        .map(<[String]>::to_vec)
        .collect();
    Ok(replay)
}

/// The text of the copied flag file: the original without trailing newlines, then one line per
/// added flag.
fn appended(original: &str, lines: &[String]) -> String {
    let mut text = original.trim_end_matches('\n').to_owned();
    text.push('\n');
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Swaps argv[IDX] of the first step for a copy of that file with the added lines.
fn apply_append(step: &mut [String], append: &Append) -> Result<(), String> {
    let missing = || format!("no file at argv[{}] to append to", append.idx);
    let slot = step.get_mut(append.idx).ok_or_else(missing)?;
    if !Path::new(slot.as_str()).is_file() {
        return Err(missing());
    }
    let original = std::fs::read_to_string(slot.as_str()).map_err(|e| format!("{slot}: {e}"))?;
    std::fs::write(&append.dest, appended(&original, &append.lines))
        .map_err(|e| format!("{}: {e}", append.dest))?;
    slot.clone_from(&append.dest);
    Ok(())
}

/// The exit code a POSIX shell reports for a finished child.
fn exit_code(status: ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    status.code().unwrap_or(1)
}

/// Runs one step with its stderr appended to `raw` and returns its exit code.
fn run_step(step: &[String], raw: &mut Vec<u8>) -> i32 {
    let Some((program, args)) = step.split_first() else {
        raw.extend_from_slice(format!("{PREFIX}empty step\n").as_bytes());
        return 2;
    };
    match Command::new(program)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => {
            raw.extend_from_slice(&output.stderr);
            exit_code(output.status)
        }
        Err(error) => {
            raw.extend_from_slice(format!("{PREFIX}cannot run {program}: {error}\n").as_bytes());
            127
        }
    }
}

/// Replaces each alias in every line of the collected stderr; each line ends in a newline.
fn aliased(raw: &str, aliases: &[(String, String)]) -> String {
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        let mut line = line.to_owned();
        for (from, to) in aliases.iter().filter(|(from, _)| !from.is_empty()) {
            line = line.replace(from.as_str(), to);
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn run(replay: &Replay) -> Result<(), String> {
    let mut raw = Vec::new();
    let mut code = 0;
    for (n, step) in replay.steps.iter().enumerate() {
        let mut step = step.clone();
        let first_append = replay.append.as_ref().filter(|_| n == 0);
        if let Some(Err(message)) = first_append.map(|append| apply_append(&mut step, append)) {
            raw.extend_from_slice(format!("{PREFIX}{message}\n").as_bytes());
            code = 2;
            break;
        }
        code = run_step(&step, &mut raw);
        if code != 0 {
            break;
        }
    }
    let log = aliased(&String::from_utf8_lossy(&raw), &replay.aliases);
    std::fs::write(&replay.log, log).map_err(|e| format!("{}: {e}", replay.log))?;
    std::fs::write(&replay.status, format!("{code}\n"))
        .map_err(|e| format!("{}: {e}", replay.status))
}
