//! Command line entry for the Protobuf docs converter.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "protobuf_extract",
        dx_docs_protobuf_extract::usage(),
        dx_docs_protobuf_extract::parse,
        dx_docs_protobuf_extract::run,
    )
}
