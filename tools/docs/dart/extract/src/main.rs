//! Command line entry for the Dart docs converter.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "dart_extract",
        dx_docs_dart_extract::usage(),
        dx_docs_dart_extract::parse,
        dx_docs_dart_extract::run,
    )
}
