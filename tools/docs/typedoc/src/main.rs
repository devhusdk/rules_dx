//! Command line entry for the TypeDoc extractor and IR renderer.

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
        println!("{}", dx_docs_typedoc::usage());
        return ExitCode::SUCCESS;
    }
    let command = match dx_docs_typedoc::parse(&args) {
        Ok(command) => command,
        Err(detail) => {
            eprintln!("docs_typedoc: {detail}\n{}", dx_docs_typedoc::usage());
            return ExitCode::from(2);
        }
    };
    match dx_docs_typedoc::run(&command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(detail) => {
            eprintln!("docs_typedoc: {detail}");
            ExitCode::FAILURE
        }
    }
}
