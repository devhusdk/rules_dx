//! Command line entry for the Swift API mapper.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "swift_extract",
        dx_docs_swift_extract::usage(),
        dx_docs_swift_extract::parse,
        dx_docs_swift_extract::run,
    )
}
