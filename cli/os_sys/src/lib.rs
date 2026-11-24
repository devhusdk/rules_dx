#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]
#![cfg_attr(
    all(test, unix),
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

pub const STOP_SIGNALS: [i32; 3] = [2, 15, 1];

pub mod console;
#[cfg(unix)]
pub mod errno;
pub mod exit;
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
#[cfg(all(test, any(target_os = "linux", target_os = "macos", windows)))]
#[path = "exit_tests.rs"]
mod exit_tests;
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
