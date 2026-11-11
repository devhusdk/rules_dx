//! Command line entry for the component metadata converter.

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
        "component_extract",
        dx_docs_component_extract::usage(),
        dx_docs_component_extract::parse,
        dx_docs_component_extract::run,
    )
}
