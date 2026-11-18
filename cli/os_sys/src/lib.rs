//! Safe wrappers over the Unix and Windows system calls dx needs.
//!
//! Every `unsafe` block in dx's process code lives in this crate. Callers get
//! plain functions that return `io::Result`; the few raw calls rustix does not cover
//! (signal masks, `sigaction`, `fork`, macOS `sysctl`) and the Win32 and ntdll calls
//! stay in single-operation blocks with a `SAFETY` comment each.

#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

/// The stop signals dx forwards to a streamed child: SIGINT, SIGTERM and SIGHUP.
pub const STOP_SIGNALS: [i32; 3] = [2, 15, 1];

pub mod console;
#[cfg(unix)]
pub mod errno;
#[cfg(unix)]
pub mod fd;
#[cfg(any(unix, windows))]
pub mod fs;
#[cfg(unix)]
pub mod job;
#[cfg(windows)]
pub mod job_object;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod nt;
#[cfg(unix)]
pub mod process;
#[cfg(unix)]
pub mod signals;
#[cfg(unix)]
pub mod spawn;
#[cfg(unix)]
pub mod tty;
#[cfg(windows)]
pub mod win32;

#[cfg(test)]
#[path = "console_tests.rs"]
mod console_tests;
#[cfg(test)]
#[path = "job_object_tests.rs"]
mod job_object_tests;
#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
#[cfg(test)]
#[path = "nt_tests.rs"]
mod nt_tests;
#[cfg(test)]
#[path = "win32_tests.rs"]
mod win32_tests;
