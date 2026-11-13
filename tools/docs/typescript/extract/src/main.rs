//! Command line entry for the TypeDoc extractor.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "typescript_extract",
        dx_docs_typescript_extract::usage(),
        dx_docs_typescript_extract::parse,
        dx_docs_typescript_extract::run,
    )
}
