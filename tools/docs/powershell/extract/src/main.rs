//! Command line entry for the PowerShell API mapper.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "powershell_extract",
        dx_docs_powershell_extract::usage(),
        dx_docs_powershell_extract::parse,
        dx_docs_powershell_extract::run,
    )
}
