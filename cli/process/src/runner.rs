//! The production `Runner`: streams children through the lifecycle layer.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::lifecycle::{
    self, CleanupBudget, EnvPolicy, StreamGroup, StreamOutcome, StreamOutput, StreamSpec,
    StreamStdin,
};
use crate::{ChildStatus, Runner};

const STREAM_CLEANUP: CleanupBudget = CleanupBudget {
    graceful: Duration::from_secs(10),
    forced: Duration::from_secs(2),
    drain: Duration::from_secs(5),
};

/// Runs children with stop forwarding; `on_stop` fires when a stop signal cancelled the child.
#[derive(Clone, Copy)]
pub struct BinaryRunner {
    inherit_stdout: bool,
    stop: fn() -> Option<i32>,
    on_stop: fn(i32),
}

fn stream_spec(
    argv: &[String],
    cwd: &Path,
    env: &[(&str, &str)],
    clear_env: bool,
    inherit_stdout: bool,
) -> StreamSpec {
    let vars = env
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect();
    StreamSpec {
        argv: argv.iter().map(Into::into).collect(),
        cwd: cwd.to_path_buf(),
        env: if clear_env {
            EnvPolicy::Controlled { vars }
        } else {
            EnvPolicy::Inherited { extra: vars }
        },
        stdin: StreamStdin::Inherit,
        stdout: if inherit_stdout {
            StreamOutput::Inherit
        } else {
            StreamOutput::Forward(Box::new(io::stderr()))
        },
        stderr: StreamOutput::Inherit,
        group: lifecycle::stream_group(),
        held: Vec::new(),
    }
}

impl BinaryRunner {
    /// A runner that stops on process interrupts and reports them to `on_stop`.
    pub fn new(inherit_stdout: bool, on_stop: fn(i32)) -> Self {
        Self {
            inherit_stdout,
            stop: crate::interrupted,
            on_stop,
        }
    }

    /// Streams `spec` under the shared cleanup budget; `None` means the run was withdrawn.
    pub fn stream(&self, spec: StreamSpec) -> io::Result<Option<ChildStatus>> {
        let outcome = lifecycle::run_streamed(spec, STREAM_CLEANUP, &self.stop)?;
        Ok(self.child_status(outcome))
    }

    fn child_status(&self, outcome: StreamOutcome) -> Option<ChildStatus> {
        match outcome {
            StreamOutcome::Exited(exit) => Some(exit.into()),
            StreamOutcome::Cancelled { signal } => {
                (self.on_stop)(signal);
                Some(ChildStatus::signalled(None))
            }
            StreamOutcome::Withdrawn => None,
        }
    }

    fn spawn_streamed(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        clear_env: bool,
    ) -> io::Result<ChildStatus> {
        let spec = stream_spec(argv, cwd, env, clear_env, self.inherit_stdout);
        Ok(self.stream(spec)?.unwrap_or(ChildStatus::signalled(None)))
    }
}

impl Runner for BinaryRunner {
    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.spawn_streamed(argv, cwd, env, true)
    }

    fn pinned_tool(&self, var: &str) -> Option<PathBuf> {
        crate::pinned_env_path(var)
    }

    fn tool_version(&self, path: &Path) -> Option<String> {
        crate::version_line(path)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        crate::disk_space(path).map(|space| space.free)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.spawn_streamed(argv, cwd, env, false)
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        let mut spec = stream_spec(argv, cwd, env, false, self.inherit_stdout);
        spec.stdin = StreamStdin::Null;
        spec.group = StreamGroup::Owned;
        let outcome = lifecycle::run_streamed_until(spec, STREAM_CLEANUP, &self.stop, withdraw)?;
        Ok(self.child_status(outcome))
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod runner_tests;
