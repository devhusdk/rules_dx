//! Writes the API pages, index, SUMMARY fragment and search records of one docs site.

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
        println!("{}", dx_site_aggregate::usage());
        return ExitCode::SUCCESS;
    }
    let options = match dx_site_aggregate::parse(&args) {
        Ok(options) => options,
        Err(detail) => {
            eprintln!("site_aggregate: {detail}\n{}", dx_site_aggregate::usage());
            return ExitCode::from(2);
        }
    };
    let error = dx_site_aggregate::run(&options);
    if error.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("docs_site: {error}");
    ExitCode::FAILURE
}
