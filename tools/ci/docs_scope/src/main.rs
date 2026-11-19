//! `docs_scope_bin` runs dx lint over the patterns in `DX_DOCS_SCOPE`.

#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    if let Some(dir) = std::env::var_os("BUILD_WORKING_DIRECTORY")
        && let Err(error) = std::env::set_current_dir(&dir)
    {
        eprintln!("dx-ci: cannot enter {}: {error}", dir.display());
        return ExitCode::FAILURE;
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
        Ok(status) => status.code().map_or(ExitCode::FAILURE, |code| {
            ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX))
        }),
        Err(error) => {
            eprintln!("dx-ci: bazel cannot start: {error}");
            ExitCode::FAILURE
        }
    }
}
