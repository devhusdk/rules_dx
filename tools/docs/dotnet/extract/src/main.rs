//! Command line entry for the C# and F# API mapper.

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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", dx_docs_dotnet_extract::usage());
        return ExitCode::SUCCESS;
    }
    let options = match dx_docs_dotnet_extract::parse(&args) {
        Ok(options) => options,
        Err(detail) => {
            eprintln!(
                "dotnet_extract: {detail}\n{}",
                dx_docs_dotnet_extract::usage()
            );
            return ExitCode::from(2);
        }
    };
    match dx_docs_dotnet_extract::run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(detail) => {
            eprintln!("dotnet_extract: {detail}");
            ExitCode::FAILURE
        }
    }
}
