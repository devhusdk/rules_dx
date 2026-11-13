//! Command line entry for the OCaml docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "ocaml_extract",
        dx_docs_ocaml_extract::usage(),
        dx_docs_ocaml_extract::parse_args,
        dx_docs_ocaml_extract::run,
    )
}
