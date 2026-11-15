//! Command line entry for the Elixir and Erlang docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use serde_json as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "beam_extract",
        dx_docs_beam_extract::usage(),
        dx_docs_beam_extract::parse,
        dx_docs_beam_extract::run,
    )
}
