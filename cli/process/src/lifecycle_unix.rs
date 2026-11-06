use super::backend::Group;
use super::{Command, Descendants, ExitStatus};
use std::io;
use std::process::{Child, ChildStderr, ChildStdout};

const SIGTERM: i32 = 15;
const SIGKILL: i32 = 9;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    fn getpgrp() -> i32;
    fn tcgetpgrp(fd: i32) -> i32;
}

pub struct OwnedChild {
    child: std::process::Child,
    descendants: Descendants,
}

impl OwnedChild {
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }
}

// LCOV_EXCL_START - reason: runs in the forked child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
/// Runs in the child between fork and exec: die with the parent, and fail if it is already gone.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn child_guard(parent: i32) -> io::Result<()> {
    unsafe extern "C" {
        fn getppid() -> i32;
    }
    #[cfg(target_os = "linux")]
    {
        const PR_SET_PDEATHSIG: i32 = 1;
        unsafe extern "C" {
            fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
        }
        unsafe { prctl(PR_SET_PDEATHSIG, SIGTERM as u64, 0, 0, 0) };
    }
    if unsafe { getppid() } != parent {
        return Err(io::Error::from(io::ErrorKind::BrokenPipe));
    }
    Ok(())
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

#[cfg(target_os = "linux")]
fn stop_with_parent(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    let parent = std::process::id() as i32;
    unsafe {
        command.pre_exec(move || child_guard(parent));
    }
}

#[cfg(not(target_os = "linux"))]
fn stop_with_parent(_command: &mut Command) {}

pub fn spawn_owned(command: &mut Command, descendants: Descendants) -> io::Result<OwnedChild> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    stop_with_parent(command);
    Ok(OwnedChild {
        child: command.spawn()?,
        descendants,
    })
}

/// Ends what an owned run left in the child's group once the child has exited.
pub fn end_owned_descendants(owned: &mut OwnedChild) {
    if owned.descendants == Descendants::Owned {
        signal_group(owned, SIGKILL);
    }
}

fn signal_group(owned: &mut OwnedChild, signo: i32) {
    let pgid = -(owned.child.id() as i32);
    unsafe {
        kill(pgid, signo);
    }
}

pub fn terminate_graceful(owned: &mut OwnedChild) {
    signal_group(owned, SIGTERM);
}

pub fn terminate_forced(owned: &mut OwnedChild) {
    signal_group(owned, SIGKILL);
}

pub fn termination_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

/// Nothing beyond the inherited descriptors keeps a Unix tree's locks.
pub struct StreamTree;

const F_GETFD: i32 = 1;
const F_SETFD: i32 = 2;
const FD_CLOEXEC: i32 = 1;

unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
}

/// Clears close-on-exec on the held descriptors in the child only,
/// so concurrent spawns from other threads never inherit them.
fn inherit_held(command: &mut Command, held: &[std::fs::File]) {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    if held.is_empty() {
        return;
    }
    let fds: Vec<i32> = held.iter().map(AsRawFd::as_raw_fd).collect();
    // SAFETY: the hook only calls fcntl on descriptors collected before
    // the fork; it allocates nothing and takes no locks.
    unsafe {
        // LCOV_EXCL_START - reason: runs in the forked child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        command.pre_exec(move || {
            for &fd in &fds {
                let flags = fcntl(fd, F_GETFD);
                if flags < 0 || fcntl(fd, F_SETFD, flags & !FD_CLOEXEC) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

pub fn spawn_streamed(
    command: &mut Command,
    group: Group,
    held: &[std::fs::File],
) -> io::Result<(Child, StreamTree)> {
    use std::os::unix::process::CommandExt;
    match group {
        Group::Owned => {
            command.process_group(0);
            stop_with_parent(command);
        }
        // LCOV_EXCL_START - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        Group::Join(pgid) => {
            command.process_group(pgid);
        }
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        Group::Own => {
            command.process_group(0);
        }
    }
    inherit_held(command, held);
    Ok((command.spawn()?, StreamTree))
}

/// The negative group id every forwarded signal goes to.
pub fn stream_target(child: &Child, group: Group) -> i32 {
    match group {
        Group::Join(pgid) => -pgid, // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        Group::Owned | Group::Own => -(child.id() as i32),
    }
}

pub fn send(target: i32, signo: i32) {
    if target != 0 {
        unsafe {
            let slot = errno_slot();
            let saved = *slot;
            kill(target, signo);
            *slot = saved;
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn errno_slot() -> *mut i32 {
    unsafe { libc::__errno_location() }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
unsafe fn errno_slot() -> *mut i32 {
    unsafe { libc::__error() }
}

pub fn send_kill(target: i32) {
    send(target, SIGKILL);
}

pub fn force_stream(_child: &mut Child, target: i32) {
    send_kill(target);
}

pub fn tree_alive(target: i32) -> bool {
    target != 0 && unsafe { kill(target, 0) } == 0
}

pub fn terminal_foreground() -> bool {
    use std::os::fd::AsRawFd;
    let Ok(tty) = std::fs::File::open("/dev/tty") else {
        return false;
    };
    let foreground = unsafe { tcgetpgrp(tty.as_raw_fd()) }; // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    foreground > 0 && foreground == unsafe { getpgrp() } // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
}
