//! Command line entry for the JVM docs converter.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "jvm_convert",
        dx_docs_jvm_convert::usage(),
        dx_docs_jvm_convert::parse,
        dx_docs_jvm_convert::run,
    )
}
