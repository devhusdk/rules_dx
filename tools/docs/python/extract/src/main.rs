//! Command line entry for the Python docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "python_extract",
        dx_docs_python_extract::usage(),
        dx_docs_python_extract::parse,
        dx_docs_python_extract::run,
    )
}
