//! Command line entry for the OCaml docs converter.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "ocaml_extract",
        dx_docs_ocaml_extract::usage(),
        dx_docs_ocaml_extract::parse_args,
        dx_docs_ocaml_extract::run,
    )
}
