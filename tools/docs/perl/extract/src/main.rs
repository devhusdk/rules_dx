//! Command line entry for the Perl docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "perl_extract",
        dx_docs_perl_extract::usage(),
        dx_docs_perl_extract::parse,
        dx_docs_perl_extract::run,
    )
}
