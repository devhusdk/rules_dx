//! Command line entry for the Haskell API mapper.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "haskell_extract",
        dx_docs_haskell_extract::usage(),
        dx_docs_haskell_extract::parse,
        dx_docs_haskell_extract::run,
    )
}
