//! Command line entry for the Haskell API mapper.

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
        "haskell_extract",
        dx_docs_haskell_extract::usage(),
        dx_docs_haskell_extract::parse,
        dx_docs_haskell_extract::run,
    )
}
