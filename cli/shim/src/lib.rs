#![forbid(unsafe_code)]
#![deny(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::string_slice
)]
#![cfg_attr(
    windows,
    expect(
        clippy::disallowed_methods,
        reason = "runs local tools that exit on their own"
    )
)]

pub mod fetch;
pub mod protocol;
pub mod run;
