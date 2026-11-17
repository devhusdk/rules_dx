//! Safe wrappers over the Unix system calls dx needs.
//!
//! Every `unsafe` block in dx's Unix process code lives in this crate. Callers get
//! plain functions that return `io::Result`; the few raw calls rustix does not cover
//! (signal masks, `sigaction`, `fork`, macOS `sysctl`) stay in single-operation
//! blocks with a `SAFETY` comment each.

#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

/// The stop signals dx forwards to a streamed child: SIGINT, SIGTERM and SIGHUP.
pub const STOP_SIGNALS: [i32; 3] = [2, 15, 1];

#[cfg(unix)]
pub mod errno;
#[cfg(unix)]
pub mod fd;
#[cfg(unix)]
pub mod fs;
#[cfg(unix)]
pub mod job;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(unix)]
pub mod process;
#[cfg(unix)]
pub mod signals;
#[cfg(unix)]
pub mod spawn;
#[cfg(unix)]
pub mod tty;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
