#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::Write;

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let mut stdout = std::io::stdout().lock();
    let code = match unified_diff::run(&args, &mut stdout).and_then(|code| {
        stdout
            .flush()
            .map(|()| code)
            .map_err(|error| format!("stdout: {error}"))
    }) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("diff: {message}");
            unified_diff::TROUBLE
        }
    };
    std::process::exit(code);
}
