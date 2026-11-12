//! Command line entry for the C, C++ and CUDA docs converter.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "cc_extract",
        dx_docs_cc_extract::usage(),
        dx_docs_cc_extract::parse,
        dx_docs_cc_extract::run,
    )
}
