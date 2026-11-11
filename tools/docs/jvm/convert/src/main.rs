//! Command line entry for the JVM docs converter.

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
        "jvm_convert",
        dx_docs_jvm_convert::usage(),
        dx_docs_jvm_convert::parse,
        dx_docs_jvm_convert::run,
    )
}
