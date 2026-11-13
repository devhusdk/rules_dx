//! Command line entry for the D API mapper.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "d_extract",
        dx_docs_d_extract::usage(),
        dx_docs_d_extract::parse,
        dx_docs_d_extract::run,
    )
}
