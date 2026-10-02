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
use std::path::PathBuf;

// LCOV_EXCL_START - reason: thin man shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    dx_output::init_diagnostics(false);
    let out: PathBuf = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            tracing::error!("usage: dx_man <output-file>");
            std::process::exit(2);
        });
    // LCOV_EXCL_STOP - reason: end thin man shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let man = clap_mangen::Man::new(dx_cli::args::cli_command()).section("1");
    let mut buffer = Vec::new();
    man.render(&mut buffer).unwrap_or_else(|error| {
        tracing::error!("dx_man: render failed: {error}");
        std::process::exit(1);
    });
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                tracing::error!("dx_man: cannot create {}: {error}", parent.display());
                std::process::exit(1);
            });
        }
    }
    let mut file = std::fs::File::create(&out).unwrap_or_else(|error| {
        tracing::error!("dx_man: cannot write {}: {error}", out.display());
        std::process::exit(1);
    });
    file.write_all(&buffer).unwrap_or_else(|error| {
        tracing::error!("dx_man: cannot write {}: {error}", out.display());
        std::process::exit(1);
    });
}
