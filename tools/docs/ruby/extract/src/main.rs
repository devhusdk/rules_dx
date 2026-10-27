//! Command line entry for the Ruby docs converter.

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
        println!("{}", dx_docs_ruby_extract::usage());
        return ExitCode::SUCCESS;
    }
    let options = match dx_docs_ruby_extract::parse(&args) {
        Ok(options) => options,
        Err(detail) => {
            eprintln!("ruby_extract: {detail}\n{}", dx_docs_ruby_extract::usage());
            return ExitCode::from(2);
        }
    };
    match dx_docs_ruby_extract::run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(detail) => {
            eprintln!("ruby_extract: {detail}");
            ExitCode::FAILURE
        }
    }
}
