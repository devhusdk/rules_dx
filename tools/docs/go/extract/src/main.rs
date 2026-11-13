//! Command line entry for the Go docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "go_extract",
        dx_docs_go_extract::usage(),
        dx_docs_go_extract::parse,
        dx_docs_go_extract::run,
    )
}
