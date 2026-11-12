//! Command line entry for the C# and F# API mapper.

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "dotnet_extract",
        dx_docs_dotnet_extract::usage(),
        dx_docs_dotnet_extract::parse,
        dx_docs_dotnet_extract::run,
    )
}
