//! Steps a spawned child takes between fork and exec.

#![expect(unsafe_code, reason = "Command::pre_exec is unsafe")]

use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Makes the child get `signo` when this process dies, and fail to spawn if it already has.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn die_with_parent(command: &mut Command, signo: i32) {
    let parent = std::process::id();
    // SAFETY: the hook calls only prctl and getppid, which are async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            crate::process::die_with_parent(signo)?;
            if std::os::unix::process::parent_id() != parent {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            Ok(())
        })
    };
}

/// Clears close-on-exec on `fds` in the child only, so concurrent spawns from
/// other threads never inherit them. Each fd shares its open file description
/// with the descriptor it was duplicated from, so a `flock` lock stays held.
pub fn inherit(command: &mut Command, fds: Vec<OwnedFd>) {
    if fds.is_empty() {
        return;
    }
    // SAFETY: the hook only calls fcntl on descriptors it owns.
    unsafe {
        command.pre_exec(move || {
            for fd in &fds {
                crate::fd::set_inheritable(fd)?;
            }
            Ok(())
        })
    };
}

/// Makes the child replace the text of `file` with `<pid> <namespace> <start> <boot>` before
/// it execs, where `<start>` is its own start time in [`crate::process::start_time`] form.
/// The child writes nothing when it cannot read its start time.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn record_start(command: &mut Command, file: std::fs::File, namespace: String, boot: String) {
    // SAFETY: the hook reads and writes only through stack buffers and raw descriptor calls,
    // which are async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            write_start_line(&file, &namespace, &boot);
            Ok(())
        })
    };
}

/// Runs between fork and exec, so it never allocates: std opens short paths from a stack buffer.
#[cfg(target_os = "linux")]
fn write_start_line(file: &std::fs::File, namespace: &str, boot: &str) {
    use std::io::Read;
    let mut stat = [0u8; 4096];
    let Ok(mut source) = std::fs::File::open("/proc/self/stat") else {
        return;
    };
    let Ok(read) = source.read(&mut stat) else {
        return;
    };
    let Some(start) = stat
        .get(..read)
        .and_then(crate::process::stat_start)
        .and_then(|start| std::str::from_utf8(start).ok())
    else {
        return;
    };
    write_line(file, format_args!("{start}"), namespace, boot);
}

/// Runs between fork and exec, so it never allocates.
#[cfg(target_os = "macos")]
fn write_start_line(file: &std::fs::File, namespace: &str, boot: &str) {
    let Ok(pid) = i32::try_from(std::process::id()) else {
        return;
    };
    let Some((secs, micros)) = crate::macos::start_timeval(pid) else {
        return;
    };
    write_line(file, format_args!("{secs}.{micros:06}"), namespace, boot);
}

/// Replaces the text of `file` with the calling process's line, using only a stack buffer.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn write_line(file: &std::fs::File, start: std::fmt::Arguments<'_>, namespace: &str, boot: &str) {
    use std::io::Write;
    use std::os::unix::fs::FileExt;
    let pid = std::process::id();
    let mut line = [0u8; 1024];
    let mut cursor = std::io::Cursor::new(&mut line[..]);
    if writeln!(cursor, "{pid} {namespace} {start} {boot}").is_err() {
        return;
    }
    let Some(text) = usize::try_from(cursor.position())
        .ok()
        .and_then(|len| line.get(..len))
    else {
        return;
    };
    // The parent rewrites the lease after spawn if this best-effort write fails.
    if file.set_len(0).is_ok() {
        drop(file.write_all_at(text, 0));
    }
}
