//! Command line entry for the Avro docs converter.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "avro_extract",
        dx_docs_avro_extract::usage(),
        dx_docs_avro_extract::parse,
        dx_docs_avro_extract::run,
    )
}
