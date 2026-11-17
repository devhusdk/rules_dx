//! A watcher-led foreground group on the parent's terminal for one streamed child.

use dx_os_sys::job::{JOB_STOPS, SETTLE, SETTLED};
use dx_os_sys::process::{SIGCONT, SIGKILL};
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Only one run in a process may hold the terminal at a time.
static HELD: AtomicBool = AtomicBool::new(false);

/// How long [`TerminalJob::settle`] waits for the watcher.
const SETTLE_WAIT: Duration = Duration::from_millis(1000);

/// Releases the one-holder guard when the job ends.
struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        HELD.store(false, Ordering::SeqCst);
    }
}

/// A child's foreground process group on the parent's terminal.
pub struct TerminalJob {
    tty: File,
    own: i32,
    group: i32,
    report: File,
    _guard: Guard,
}

// LCOV_EXCL_START - reason: needs a controlling terminal in the foreground, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
impl TerminalJob {
    /// Starts a watcher-led foreground group, or `None` when this process
    /// has no terminal in the foreground or any step fails.
    pub fn start() -> Option<TerminalJob> {
        if HELD.swap(true, Ordering::SeqCst) {
            return None;
        }
        let guard = Guard;
        let tty = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        let own = dx_os_sys::process::own_group();
        if dx_os_sys::tty::foreground_group(&tty).ok()? != own {
            return None;
        }
        let (read, write) = dx_os_sys::fd::pipe_nonblocking().ok()?;
        let pid = dx_os_sys::job::spawn_watcher(write).ok()?;
        let job = TerminalJob {
            tty,
            own,
            group: pid,
            report: File::from(read),
            _guard: guard,
        };
        // The watcher also joins its own group; whichever side runs first wins the race.
        if dx_os_sys::process::set_group(pid, pid).is_err()
            && dx_os_sys::process::group_of(pid).ok() != Some(pid)
        {
            return None;
        }
        dx_os_sys::tty::set_foreground_group(&job.tty, pid)
            .ok()
            .map(|()| job)
    }

    /// The foreground group the child joins.
    pub fn pgid(&self) -> i32 {
        self.group
    }

    /// The next stop signal the terminal delivered to the group, if any.
    pub fn take_reported(&self) -> Option<i32> {
        let mut byte = [SETTLED];
        let read = (&self.report).read(&mut byte).ok()?;
        let [signo] = byte;
        (read == 1 && signo != SETTLED).then_some(i32::from(signo))
    }

    /// Returns every stop signal the group got before this call, once the watcher confirms it reported them all.
    pub fn settle(&self) -> Vec<i32> {
        let mut reported = Vec::new();
        if dx_os_sys::process::signal(self.group, SETTLE).is_err() {
            return reported;
        }
        loop {
            if !dx_os_sys::fd::wait_readable(&self.report, SETTLE_WAIT).unwrap_or(false) {
                return reported;
            }
            let mut byte = [SETTLED];
            let read = (&self.report).read(&mut byte);
            match (read, byte) {
                (Ok(1), [SETTLED]) => return reported,
                (Ok(1), [signo]) => reported.push(i32::from(signo)),
                (Ok(_) | Err(_), _) => return reported,
            }
        }
    }

    /// Follows a job-control stop of the child: hands the terminal back,
    /// stops this process like the shell expects, then resumes the group.
    pub fn tend(&self, child: i32) {
        let Ok(Some(signo)) = dx_os_sys::process::stopped_by(child) else {
            return;
        };
        if !JOB_STOPS.contains(&signo) {
            return;
        }
        self.hand_over(self.group, self.own);
        // Nothing to undo when the stop fails: dx just keeps running in the background.
        drop(dx_os_sys::process::stop_self());
        self.hand_over(self.own, self.group);
        drop(dx_os_sys::process::signal(-self.group, SIGCONT));
    }

    /// Moves the terminal to `to` only while `from` still holds it.
    fn hand_over(&self, from: i32, to: i32) {
        if dx_os_sys::tty::foreground_group(&self.tty).is_ok_and(|group| group == from) {
            // The terminal stays where it is if the move fails; there is no better place for it.
            drop(dx_os_sys::tty::set_foreground_group(&self.tty, to));
        }
    }
}

impl Drop for TerminalJob {
    fn drop(&mut self) {
        self.hand_over(self.group, self.own);
        drop(dx_os_sys::process::signal(self.group, SIGKILL));
        drop(dx_os_sys::process::reap(self.group));
    }
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

pub use dx_os_sys::signals::allow_background_tty_writes;
