mod git;
pub(crate) mod hook;
pub(crate) mod host;
pub(crate) mod ops;
pub(crate) mod pool;
pub(crate) mod setup;
#[cfg(test)]
mod tests;

pub use hook::HookKind;
pub(crate) use pool::{lease_for_workspace, Who};

/// Why a worktree command stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorktreeError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) usage: bool,
}

impl WorktreeError {
    pub(crate) fn usage(code: &'static str, message: &str) -> WorktreeError {
        WorktreeError {
            code,
            message: message.to_owned(),
            usage: true,
        }
    }

    pub(crate) fn refused(code: &'static str, message: &str) -> WorktreeError {
        WorktreeError {
            code,
            message: message.to_owned(),
            usage: false,
        }
    }

    pub(crate) fn git(args: &[&str], stderr: &str) -> WorktreeError {
        let command = args.first().copied().unwrap_or("git");
        WorktreeError::refused("git_failed", &format!("git {command} failed: {stderr}"))
    }

    pub(crate) fn io(error: std::io::Error) -> WorktreeError {
        WorktreeError::refused("io_failed", &error.to_string())
    }

    pub(crate) fn exit(&self) -> i32 {
        if self.usage {
            dx_process::pre_exec_code()
        } else {
            dx_process::operational_code()
        }
    }
}

impl std::fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
