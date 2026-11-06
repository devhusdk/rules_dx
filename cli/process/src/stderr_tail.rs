//! The last stretch of a Bazel run's stderr, kept so a failure can be classified.

use std::io::{self, IsTerminal, Write};
use std::sync::Mutex;

const KEEP_BYTES: usize = 256 * 1024;

static TAIL: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Forgets the previous run's stderr.
pub(crate) fn clear() {
    TAIL.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// The stderr of the latest Bazel run; empty before any run.
pub fn last_bazel_stderr() -> String {
    let tail = TAIL.lock().unwrap_or_else(|e| e.into_inner());
    String::from_utf8_lossy(&tail).into_owned()
}

/// Whether stderr of this run should be copied: only for Bazel.
pub(crate) fn wanted(program: &std::ffi::OsStr) -> bool {
    let name = std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.starts_with("bazel")
}

/// Bazel turns colors and curses progress off when stderr is a pipe. When dx copies
/// stderr on a terminal, this puts them back right after the command word, unless
/// the user already chose.
pub(crate) fn keep_terminal_ui(argv: &[String]) -> Vec<String> {
    keep_ui(argv, io::stderr().is_terminal())
}

fn keep_ui(argv: &[String], terminal: bool) -> Vec<String> {
    let command = argv.iter().skip(1).position(|arg| !arg.starts_with('-'));
    let chosen = |flag: &str| argv.iter().any(|arg| arg.starts_with(flag));
    let Some(command) = command.map(|at| at + 1) else {
        return argv.to_vec();
    };
    if !terminal || !wanted(argv[0].as_ref()) {
        return argv.to_vec();
    }
    let mut out = argv[..=command].to_vec();
    if !chosen("--color") {
        out.push("--color=yes".to_owned());
    }
    if !chosen("--curses") {
        out.push("--curses=yes".to_owned());
    }
    out.extend_from_slice(&argv[command + 1..]);
    out
}

/// Writes to stderr and keeps the last `KEEP_BYTES` written.
pub(crate) struct Tee;

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = io::stderr().write(buf)?;
        push(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

fn push(bytes: &[u8]) {
    let mut tail = TAIL.lock().unwrap_or_else(|e| e.into_inner());
    tail.extend_from_slice(bytes);
    if tail.len() > KEEP_BYTES {
        let drop = tail.len() - KEEP_BYTES;
        tail.drain(..drop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_last_bytes() {
        clear();
        push(b"first ");
        push(&vec![b'x'; KEEP_BYTES]);
        let kept = last_bazel_stderr();
        assert_eq!(kept.len(), KEEP_BYTES);
        assert!(!kept.contains("first"));
        clear();
        assert_eq!(last_bazel_stderr(), "");
    }

    #[test]
    fn the_tee_writes_every_byte_through() {
        let mut tee = Tee;
        assert_eq!(tee.write(b"tee\n").expect("write"), 4);
        tee.flush().expect("flush");
    }

    #[test]
    fn terminal_ui_is_kept_after_the_command_word() {
        let argv: Vec<String> = ["bazel", "--output_base=/o", "build", "//a", "--", "x"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            keep_ui(&argv, true),
            [
                "bazel",
                "--output_base=/o",
                "build",
                "--color=yes",
                "--curses=yes",
                "//a",
                "--",
                "x"
            ]
        );
        assert_eq!(keep_ui(&argv, false), argv);
    }

    #[test]
    fn the_users_ui_choice_and_other_programs_are_left_alone() {
        let argv: Vec<String> = ["bazel", "build", "--color=no", "--curses=no", "//a"]
            .map(String::from)
            .to_vec();
        assert_eq!(keep_ui(&argv, true), argv);
        let git: Vec<String> = ["git", "status"].map(String::from).to_vec();
        assert_eq!(keep_ui(&git, true), git);
    }

    #[test]
    fn only_bazel_programs_are_copied() {
        assert!(!wanted(std::ffi::OsStr::new("/usr/bin/git")));
    }
}
