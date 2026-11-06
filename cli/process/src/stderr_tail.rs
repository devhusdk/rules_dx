//! The last stretch of a Bazel run's stderr, kept so a failure can be classified.

use std::io::{self, IsTerminal, Write};
use std::sync::Mutex;

const KEEP_BYTES: usize = 256 * 1024;

static TAIL: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Forgets the previous run's stderr.
pub(crate) fn clear() {
    TAIL.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// The stderr of the latest Bazel run that was not on a terminal; empty otherwise.
pub fn last_bazel_stderr() -> String {
    let tail = TAIL.lock().unwrap_or_else(|e| e.into_inner());
    String::from_utf8_lossy(&tail).into_owned()
}

/// Whether stderr of this run should be copied: only for Bazel, and never on a terminal,
/// where Bazel must keep writing to the tty itself for colors and progress.
pub(crate) fn wanted(program: &std::ffi::OsStr) -> bool {
    let name = std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.starts_with("bazel") && !io::stderr().is_terminal()
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
    fn only_bazel_programs_are_copied() {
        assert!(!wanted(std::ffi::OsStr::new("/usr/bin/git")));
    }
}
