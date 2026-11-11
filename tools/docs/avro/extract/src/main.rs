//! Command line entry for the Avro docs converter.

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
        "avro_extract",
        dx_docs_avro_extract::usage(),
        dx_docs_avro_extract::parse,
        dx_docs_avro_extract::run,
    )
}
