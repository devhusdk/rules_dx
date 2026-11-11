//! Command line entry for the Swift API mapper.

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
        "swift_extract",
        dx_docs_swift_extract::usage(),
        dx_docs_swift_extract::parse,
        dx_docs_swift_extract::run,
    )
}
