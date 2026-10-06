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
    if args.iter().any(|arg| arg == "--rendered") {
        return match parse_rendered(&args) {
            Ok((rendered, forbid)) => {
                report(dx_site_check::rendered::check_rendered(&rendered, &forbid))
            }
            Err(detail) => reject(detail, dx_site_check::rendered::usage()),
        };
    }
    match dx_site_check::parse_args(&args) {
        Ok(inputs) => report(dx_site_check::check(&inputs)),
        Err((detail, usage)) => reject(detail, usage),
    }
}

fn parse_rendered(
    args: &[String],
) -> Result<(dx_site_check::rendered::Rendered, Vec<String>), String> {
    let mut rendered = dx_site_check::rendered::Rendered::default();
    let mut forbid: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].clone();
        if flag == "--landing" {
            rendered.landing = true;
            index += 1;
            continue;
        }
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a DIR"));
        };
        match flag.as_str() {
            "--rendered" => rendered.root = std::path::PathBuf::from(value),
            "--forbid" => forbid.push(value.clone()),
            other => return Err(format!("unknown argument '{other}'")),
        }
        index += 2;
    }
    if rendered.root.as_os_str().is_empty() {
        return Err("--rendered DIR is required".to_string());
    }
    Ok((rendered, forbid))
}

fn report(refusal: String) -> ExitCode {
    if refusal.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("docs_site: {refusal}");
    ExitCode::FAILURE
}

fn reject(detail: impl std::fmt::Display, usage: impl std::fmt::Display) -> ExitCode {
    eprintln!("site_check: {detail}\n{usage}");
    ExitCode::from(2)
}
