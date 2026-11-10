//! Command line entry for the Zig API mapper.

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
        "zig_extract",
        dx_docs_zig_extract::usage(),
        dx_docs_zig_extract::parse,
        dx_docs_zig_extract::run,
    )
}
