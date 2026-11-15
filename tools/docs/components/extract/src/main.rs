//! Command line entry for the component metadata converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use serde_json as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "component_extract",
        dx_docs_component_extract::usage(),
        dx_docs_component_extract::parse,
        dx_docs_component_extract::run,
    )
}
