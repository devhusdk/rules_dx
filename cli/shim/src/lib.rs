#![forbid(unsafe_code)]
#![deny(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::string_slice
)]

pub mod fetch;
pub mod protocol;
pub mod run;
