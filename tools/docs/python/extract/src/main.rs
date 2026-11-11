//! Command line entry for the Python docs converter.

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
        "python_extract",
        dx_docs_python_extract::usage(),
        dx_docs_python_extract::parse,
        dx_docs_python_extract::run,
    )
}
