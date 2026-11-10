//! `docs_scope_bin` runs dx lint over the patterns in `DX_DOCS_SCOPE`.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    if let Some(dir) = std::env::var_os("BUILD_WORKING_DIRECTORY") {
        if let Err(error) = std::env::set_current_dir(&dir) {
            eprintln!("dx-ci: cannot enter {dir:?}: {error}");
            return ExitCode::FAILURE;
        }
    }
    let scope = std::env::var(docs_scope::SCOPE_ENV).unwrap_or_default();
    let mut config = Vec::new();
    for name in ["BAZEL_CONFIG", "BB_ARGS"] {
        let value = std::env::var(name).unwrap_or_default();
        config.extend(value.split_whitespace().map(str::to_string));
    }
    let argv = match docs_scope::lint_argv(&scope, &config) {
        Ok(argv) => argv,
        Err(message) => {
            eprintln!("dx-ci: {message}");
            return ExitCode::FAILURE;
        }
    };
    match Command::new("bazel").args(&argv).status() {
        Ok(status) => status
            .code()
            .map_or(ExitCode::FAILURE, |code| ExitCode::from(code as u8)),
        Err(error) => {
            eprintln!("dx-ci: bazel cannot start: {error}");
            ExitCode::FAILURE
        }
    }
}
