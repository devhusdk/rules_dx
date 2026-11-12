//! Command line entry for the Julia docs converter.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "julia_extract",
        dx_docs_julia_extract::usage(),
        dx_docs_julia_extract::parse,
        dx_docs_julia_extract::run,
    )
}
