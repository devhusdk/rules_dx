//! Command line entry for the TypeDoc extractor.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "typescript_extract",
        dx_docs_typescript_extract::usage(),
        dx_docs_typescript_extract::parse,
        dx_docs_typescript_extract::run,
    )
}
