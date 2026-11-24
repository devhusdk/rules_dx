use super::{Command, Descendants, ExitStatus};
use dx_os_sys::console::{self, CtrlEvent, CtrlHandler};
use dx_os_sys::job_object::{JobObject, Limit};
use dx_os_sys::win32::{CREATE_BREAKAWAY_FROM_JOB, CREATE_SUSPENDED, resume_threads};
use std::io;
use std::os::windows::io::AsHandle;
use std::process::{Child, ChildStderr, ChildStdout};

const ERROR_ACCESS_DENIED: i32 = 5;

pub(crate) struct RecordStop;

impl CtrlHandler for RecordStop {
    fn handle(event: CtrlEvent) -> bool {
        match super::console_stop_signal(event) {
            Some(signo) => {
                crate::record_interrupt(signo);
                true
            }
            None => false,
        }
    }
}

pub fn install_console_stop() -> io::Result<()> {
    console::restore_ctrl_c()?;
    console::add_handler::<RecordStop>()
}

struct Job {
    job: JobObject,
    descendants: Descendants,
}

impl Job {
    fn create(descendants: Descendants) -> io::Result<Job> {
        let limit = match descendants {
            Descendants::Owned | Descendants::Foreground => Limit::KillOnClose,
            Descendants::Detachable => Limit::BreakawayOk,
        };
        Ok(Job {
            job: JobObject::new(limit)?,
            descendants,
        })
    }

    fn start(&self, child: &Child) -> io::Result<()> {
        self.job.assign(child.as_handle())?;
        resume_threads(child.id())
    }

    fn terminate(&self) {
        drop(self.job.terminate(1));
    }

    fn stop_attached(&self) -> bool {
        let members = self.job.process_ids().ok();
        let console = console::process_ids();
        let targets = super::attached_targets(std::process::id(), members.as_deref(), &console);
        for pid in targets.iter().flatten() {
            drop(self.job.terminate_member(*pid));
        }
        targets.is_some()
    }

    fn stop(&self) {
        if self.descendants != Descendants::Detachable || !self.stop_attached() {
            self.terminate();
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if self.descendants == Descendants::Detachable {
            self.stop_attached();
        }
    }
}

pub struct OwnedChild {
    child: std::process::Child,
    job: Job,
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

pub fn spawn_owned(command: &mut Command, descendants: Descendants) -> io::Result<OwnedChild> {
    use std::os::windows::process::CommandExt;
    let job = Job::create(descendants)?;
    let mut child = match descendants {
        Descendants::Owned | Descendants::Foreground => {
            command.creation_flags(CREATE_SUSPENDED).spawn()?
        }
        Descendants::Detachable => match command
            .creation_flags(CREATE_SUSPENDED | CREATE_BREAKAWAY_FROM_JOB)
            .spawn()
        {
            Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
                command.creation_flags(CREATE_SUSPENDED).spawn()?
            }
            other => other?, // LCOV_EXCL_LINE - reason: bazel test runs inside a job that forbids breakaway, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        },
    };
    if let Err(err) = job.start(&child) {
        // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        job.terminate();
        drop(child.kill());
        drop(child.wait());
        return Err(err);
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    Ok(OwnedChild { child, job })
}

pub struct StreamTree {
    _job: Option<Job>,
}

pub fn spawn_streamed(
    command: &mut Command,
    group: super::backend::Group,
    held: &[std::fs::File],
) -> io::Result<(std::process::Child, StreamTree)> {
    use std::os::windows::process::CommandExt;
    if held.is_empty() && !matches!(group, super::backend::Group::Owned) {
        return Ok((command.spawn()?, StreamTree { _job: None }));
    }
    let job = Job::create(Descendants::Owned)?;
    let mut child = command.creation_flags(CREATE_SUSPENDED).spawn()?;
    if let Err(err) = job.start(&child) {
        // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        job.terminate();
        drop(child.kill());
        drop(child.wait());
        return Err(err);
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    Ok((child, StreamTree { _job: Some(job) }))
}

pub fn terminate_graceful(owned: &mut OwnedChild) {
    terminate_forced(owned);
}

pub fn terminate_forced(owned: &mut OwnedChild) {
    owned.job.stop();
    drop(owned.child.kill());
}

pub fn termination_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

pub fn stream_target(_child: &Child, _group: super::backend::Group) -> i32 {
    0
}

pub fn send(_target: i32, _signo: i32) {}

pub fn send_kill(_target: i32) {}

pub fn force_stream(child: &mut Child, _target: i32) {
    drop(child.kill());
}

pub fn tree_alive(_target: i32) -> bool {
    false
}

pub fn terminal_foreground() -> bool {
    true
}

pub fn allow_background_tty_writes() {}

pub struct TerminalJob;

impl TerminalJob {
    pub fn start() -> Option<TerminalJob> {
        None
    }

    pub fn pgid(&self) -> i32 {
        0
    }

    pub fn take_reported(&self) -> Option<i32> {
        None
    }

    pub fn settle(&self) -> Vec<i32> {
        Vec::new()
    }

    pub fn tend(&self, _child: i32) {}
}

pub fn end_owned_descendants(owned: &mut OwnedChild) {
    if owned.job.descendants == Descendants::Owned {
        owned.job.terminate();
    }
}
