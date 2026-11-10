//! Command line entry for the Go docs converter.

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
        "go_extract",
        dx_docs_go_extract::usage(),
        dx_docs_go_extract::parse,
        dx_docs_go_extract::run,
    )
}
