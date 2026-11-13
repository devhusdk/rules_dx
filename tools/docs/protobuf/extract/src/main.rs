//! Command line entry for the Protobuf docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "protobuf_extract",
        dx_docs_protobuf_extract::usage(),
        dx_docs_protobuf_extract::parse,
        dx_docs_protobuf_extract::run,
    )
}
