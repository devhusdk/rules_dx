//! Command line entry for the Ruby docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "ruby_extract",
        dx_docs_ruby_extract::usage(),
        dx_docs_ruby_extract::parse,
        dx_docs_ruby_extract::run,
    )
}
