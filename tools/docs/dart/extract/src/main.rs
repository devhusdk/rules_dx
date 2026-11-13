//! Command line entry for the Dart docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "dart_extract",
        dx_docs_dart_extract::usage(),
        dx_docs_dart_extract::parse,
        dx_docs_dart_extract::run,
    )
}
