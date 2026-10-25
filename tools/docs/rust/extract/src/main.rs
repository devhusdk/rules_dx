//! Writes one DocIr shard from nightly rustdoc JSON.

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
        println!("{}", dx_docs_rust_extract::usage());
        return ExitCode::SUCCESS;
    }
    let options = match dx_docs_rust_extract::parse(&args) {
        Ok(options) => options,
        Err(detail) => {
            eprintln!("rust_extract: {detail}\n{}", dx_docs_rust_extract::usage());
            return ExitCode::from(2);
        }
    };
    let error = dx_docs_rust_extract::run(&options);
    if error.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("docs_rust: {error}");
    ExitCode::FAILURE
}
