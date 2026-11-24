#![expect(unsafe_code, reason = "Command::pre_exec is unsafe")]

use std::ffi::CString;
use std::os::fd::{IntoRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn die_with_parent(command: &mut Command, signo: i32) {
    let parent = std::process::id();
    // SAFETY: the hook calls only prctl and getppid, which are async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            crate::process::die_with_parent(signo)?;
            if std::os::unix::process::parent_id() != parent {
                return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
            }
            Ok(())
        })
    };
}

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

pub fn in_terminal(command: &mut Command, tty: CString) {
    // SAFETY: the hook calls only setsid, open and ioctl, which are async-signal-safe,
    // and opens `tty` through its existing C string without allocating.
    unsafe {
        command.pre_exec(move || {
            rustix::process::setsid()?;
            let fd = rustix::fs::open(
                tty.as_c_str(),
                rustix::fs::OFlags::RDWR,
                rustix::fs::Mode::empty(),
            )?;
            rustix::process::ioctl_tiocsctty(&fd)?;
            let _kept_across_exec = fd.into_raw_fd();
            Ok(())
        })
    };
}

pub fn ignore_signal(command: &mut Command, signo: i32) {
    // SAFETY: the hook calls only signal, which is async-signal-safe.
    unsafe { command.pre_exec(move || crate::signals::ignore(signo)) };
}

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
    if file.set_len(0).is_ok() {
        drop(file.write_all_at(text, 0));
    }
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod spawn_tests;
