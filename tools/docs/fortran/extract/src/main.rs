//! Command line entry for the Fortran docs converter.

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
        "fortran_extract",
        dx_docs_fortran_extract::usage(),
        dx_docs_fortran_extract::parse,
        dx_docs_fortran_extract::run,
    )
}
