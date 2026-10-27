//! The production `Runner`: streams children through the lifecycle layer.

use std::fs::File;
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
    held: Vec<File>,
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
        held,
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
        let spec = stream_spec(argv, cwd, env, clear_env, self.inherit_stdout, Vec::new());
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
        let version = crate::version_line(path, &self.stop);
        if version.is_none() {
            if let Some(signal) = (self.stop)() {
                (self.on_stop)(signal);
            }
        }
        version
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        crate::disk_space(path).map(|space| space.free)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.spawn_streamed(argv, cwd, env, false)
    }

    fn run_app(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: Option<&dyn Fn() -> bool>,
        held: &[&File],
    ) -> io::Result<Option<ChildStatus>> {
        let held = held
            .iter()
            .map(|file| file.try_clone())
            .collect::<io::Result<Vec<File>>>()?;
        let mut spec = stream_spec(argv, cwd, env, false, self.inherit_stdout, held);
        let Some(withdraw) = withdraw else {
            return self.stream(spec);
        };
        spec.stdin = StreamStdin::Null;
        spec.group = StreamGroup::Owned;
        let outcome = lifecycle::run_streamed_until(spec, STREAM_CLEANUP, &self.stop, withdraw)?;
        Ok(self.child_status(outcome))
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        let mut spec = stream_spec(argv, cwd, env, false, self.inherit_stdout, Vec::new());
        spec.stdin = StreamStdin::Null;
        spec.group = StreamGroup::Owned;
        let outcome = lifecycle::run_streamed_until(spec, STREAM_CLEANUP, &self.stop, withdraw)?;
        Ok(self.child_status(outcome))
    }
}

/// Wraps a runner so `run_app` children also carry `held`; other runs get none.
pub struct Holding<'a> {
    inner: &'a dyn Runner,
    held: Vec<&'a File>,
}

impl<'a> Holding<'a> {
    pub fn new(inner: &'a dyn Runner, held: Vec<&'a File>) -> Self {
        Self { inner, held }
    }
}

impl Runner for Holding<'_> {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.inner.run(argv, cwd, env)
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.inner.run_hermetic(argv, cwd, env)
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        self.inner.run_withdrawable(argv, cwd, env, withdraw)
    }

    fn run_app(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: Option<&dyn Fn() -> bool>,
        held: &[&File],
    ) -> io::Result<Option<ChildStatus>> {
        let all: Vec<&File> = self
            .held
            .iter()
            .copied()
            .chain(held.iter().copied())
            .collect();
        self.inner.run_app(argv, cwd, env, withdraw, &all)
    }

    fn pinned_tool(&self, var: &str) -> Option<PathBuf> {
        self.inner.pinned_tool(var)
    }

    fn tool_version(&self, path: &Path) -> Option<String> {
        self.inner.tool_version(path)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod runner_tests;
