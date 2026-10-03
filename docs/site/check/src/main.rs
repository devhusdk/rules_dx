//! Reports whether a docs aggregate's links all resolve.

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
        println!("{}", dx_site_check::usage());
        return ExitCode::SUCCESS;
    }
    let inputs = match dx_site_check::parse_args(&args) {
        Ok(inputs) => inputs,
        Err((detail, usage)) => {
            eprintln!("site_check: {detail}\n{usage}");
            return ExitCode::from(2);
        }
    };
    let refusal = dx_site_check::check(&inputs);
    if refusal.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("docs_site: {refusal}");
    ExitCode::FAILURE
}
