//! Writes one `DocIr` shard from nightly rustdoc JSON.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use rustdoc_types as _;
use serde_json as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "docs_rust",
        dx_docs_rust_extract::usage(),
        dx_docs_rust_extract::parse,
        |options| match dx_docs_rust_extract::run(options) {
            error if error.is_empty() => Ok(()),
            error => Err(error),
        },
    )
}
