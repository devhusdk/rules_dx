//! Command line entry for the Elixir and Erlang docs converter.

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
        "beam_extract",
        dx_docs_beam_extract::usage(),
        dx_docs_beam_extract::parse,
        dx_docs_beam_extract::run,
    )
}
