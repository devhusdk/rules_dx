#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! Warm checkout slots for `dx worktree`: the slot pool and its leases, the
//! Claude Code worktree hooks, and the disk reclaim that runs before a build.

mod cache;
pub mod disk;
mod git;
pub mod hook;
pub mod host;
pub mod ops;
pub mod pool;
pub mod reap;
pub mod setup;
#[cfg(test)]
mod tests;

use std::path::Path;

pub use hook::HookKind;
pub use pool::{hold_busy, lease_for_workspace, Who};

/// Reclaims output bases before a build on low disk (without a pool, only host-wide leftovers); returns a message only when it acted or failed.
pub fn reclaim_for_build(workspace: &Path) -> Option<String> {
    let env = |name: &str| std::env::var(name).ok();
    let cache_base = pool::cache_base(&env).ok()?;
    reclaim_for_build_with(workspace, &cache_base, &host::SystemHost)
}

fn reclaim_for_build_with(
    workspace: &Path,
    cache_base: &Path,
    host: &dyn host::Host,
) -> Option<String> {
    let low = ops::low_disk(host, cache_base)?;
    let Some(pool) = pool::Pool::find(workspace, cache_base) else {
        // No pool: only host-wide leftovers (orphans, downloads, nested cases) can go; nothing is created.
        let reclaimed = ops::reclaim_unpooled(host, low);
        return reclaimed.removed.then_some(reclaimed.warning.message);
    };
    match ops::reclaim_if_low(&pool, host, pool.slot_of(workspace)) {
        Ok(reclaimed) => reclaimed
            .filter(|reclaimed| reclaimed.removed)
            .map(|reclaimed| reclaimed.warning.message),
        Err(error) => Some(ops::reclaim_failed(&error).message),
    }
}

/// Why a worktree command stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeError {
    pub(crate) code: &'static str,
    pub message: String,
    pub usage: bool,
    pub signal: Option<i32>,
}

impl WorktreeError {
    pub fn usage(code: &'static str, message: &str) -> WorktreeError {
        WorktreeError {
            code,
            message: message.to_owned(),
            usage: true,
            signal: None,
        }
    }

    pub(crate) fn refused(code: &'static str, message: &str) -> WorktreeError {
        WorktreeError {
            code,
            message: message.to_owned(),
            usage: false,
            signal: None,
        }
    }

    /// A stop by `signo` before the command finished.
    pub fn stopped(signo: i32) -> WorktreeError {
        WorktreeError {
            code: "cancelled",
            message: format!("stopped by signal {signo}"),
            usage: false,
            signal: Some(signo),
        }
    }

    pub(crate) fn git(args: &[&str], stderr: &str) -> WorktreeError {
        let command = args.first().copied().unwrap_or("git");
        WorktreeError::refused("git_failed", &format!("git {command} failed: {stderr}"))
    }

    pub(crate) fn io(error: std::io::Error) -> WorktreeError {
        WorktreeError::refused("io_failed", &error.to_string())
    }

    pub fn exit(&self) -> i32 {
        if let Some(signo) = self.signal {
            dx_process::interrupted_code(signo)
        } else if self.usage {
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
