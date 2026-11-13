//! Command line entry for the Zig API mapper.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "zig_extract",
        dx_docs_zig_extract::usage(),
        dx_docs_zig_extract::parse,
        dx_docs_zig_extract::run,
    )
}
