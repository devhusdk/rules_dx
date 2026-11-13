//! Command line entry for the PHP docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "php_extract",
        dx_docs_php_extract::usage(),
        dx_docs_php_extract::parse,
        dx_docs_php_extract::run,
    )
}
