//! Command line entry for the PowerShell API mapper.

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
        "powershell_extract",
        dx_docs_powershell_extract::usage(),
        dx_docs_powershell_extract::parse,
        dx_docs_powershell_extract::run,
    )
}
