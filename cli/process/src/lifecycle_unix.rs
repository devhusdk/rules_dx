use super::backend::Group;
use super::{Command, Descendants, ExitStatus};
use dx_os_sys::process::{Presence, SIGKILL, SIGTERM};
use std::io;
use std::process::{Child, ChildStderr, ChildStdout};

pub struct OwnedChild {
    child: std::process::Child,
    descendants: Descendants,
}

impl OwnedChild {
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn take_stdin(&mut self) -> Option<std::process::ChildStdin> {
        self.child.stdin.take()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }
}

/// Makes the child die with dx, and fail to spawn if dx is already gone.
#[cfg(target_os = "linux")]
fn stop_with_parent(command: &mut Command) {
    dx_os_sys::spawn::die_with_parent(command, SIGTERM);
}

#[cfg(not(target_os = "linux"))]
fn stop_with_parent(_command: &mut Command) {}

pub fn spawn_owned(command: &mut Command, descendants: Descendants) -> io::Result<OwnedChild> {
    use std::os::unix::process::CommandExt;
    if descendants != Descendants::Foreground {
        command.process_group(0);
    }
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
    let id = owned.child.id().cast_signed();
    let target = if owned.descendants == Descendants::Foreground {
        id
    } else {
        -id
    };
    // The child may have exited and been reaped already; there is nothing left to signal then.
    drop(dx_os_sys::process::signal(target, signo));
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

/// Clears close-on-exec on duplicates of the held descriptors in the child only,
/// so concurrent spawns from other threads never inherit them.
fn inherit_held(command: &mut Command, held: &[std::fs::File]) -> io::Result<()> {
    use std::os::fd::AsFd;
    let fds = held
        .iter()
        .map(|file| file.as_fd().try_clone_to_owned())
        .collect::<io::Result<Vec<_>>>()?;
    dx_os_sys::spawn::inherit(command, fds);
    Ok(())
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
    inherit_held(command, held)?;
    Ok((command.spawn()?, StreamTree))
}

/// The negative group id every forwarded signal goes to.
pub fn stream_target(child: &Child, group: Group) -> i32 {
    match group {
        Group::Join(pgid) => -pgid, // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        Group::Owned | Group::Own => -child.id().cast_signed(),
    }
}

/// Signals `target`; safe to call from a signal handler, and leaves `errno` as it was.
pub fn send(target: i32, signo: i32) {
    if target != 0 {
        let _errno = dx_os_sys::errno::ErrnoGuard::save();
        // A target that is already gone needs no signal.
        drop(dx_os_sys::process::signal(target, signo));
    }
}

pub fn send_kill(target: i32) {
    send(target, SIGKILL);
}

pub fn force_stream(_child: &mut Child, target: i32) {
    send_kill(target);
}

pub fn tree_alive(target: i32) -> bool {
    target != 0 && dx_os_sys::process::probe(target) == Presence::Reachable
}

pub fn terminal_foreground() -> bool {
    let Ok(tty) = std::fs::File::open("/dev/tty") else {
        return false;
    };
    dx_os_sys::tty::foreground_group(&tty) // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        .is_ok_and(|foreground| foreground == dx_os_sys::process::own_group()) // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
}
