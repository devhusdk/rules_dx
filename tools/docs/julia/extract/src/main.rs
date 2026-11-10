//! Command line entry for the Julia docs converter.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "julia_extract",
        dx_docs_julia_extract::usage(),
        dx_docs_julia_extract::parse,
        dx_docs_julia_extract::run,
    )
}
