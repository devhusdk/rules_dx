//! Assembles one mdBook source tree and runs the pinned mdBook build.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", dx_site_build::usage());
        return ExitCode::SUCCESS;
    }
    let options = match dx_site_build::parse(&args) {
        Ok(options) => options,
        Err(detail) => {
            eprintln!("site_build: {detail}\n{}", dx_site_build::usage());
            return ExitCode::from(2);
        }
    };
    let refusal = dx_site_build::run(&options);
    if refusal.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("site_build: {refusal}");
    ExitCode::FAILURE
}
