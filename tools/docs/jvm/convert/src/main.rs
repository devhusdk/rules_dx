//! Command line entry for the JVM docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "jvm_convert",
        dx_docs_jvm_convert::usage(),
        dx_docs_jvm_convert::parse,
        dx_docs_jvm_convert::run,
    )
}
