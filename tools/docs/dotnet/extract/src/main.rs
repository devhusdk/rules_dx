//! Command line entry for the C# and F# API mapper.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use serde as _;
use serde_json as _;
use sha2 as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "dotnet_extract",
        dx_docs_dotnet_extract::usage(),
        dx_docs_dotnet_extract::parse,
        dx_docs_dotnet_extract::run,
    )
}
