//! Command line entry for the Ruby docs converter.

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
        "ruby_extract",
        dx_docs_ruby_extract::usage(),
        dx_docs_ruby_extract::parse,
        dx_docs_ruby_extract::run,
    )
}
