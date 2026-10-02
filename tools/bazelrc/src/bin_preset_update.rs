#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
use std::path::PathBuf;

fn run(argv: &[String]) -> i32 {
    let verify_only = argv.len() == 2 && argv[1] == "--verify-only";
    if argv.len() != 1 && !verify_only {
        return dx_preset::bin_usage();
    }
    let workspace = match dx_preset::resolve_workspace() {
        Ok(workspace) => workspace,
        Err(error) => {
            return dx_preset::bin_cannot(format!(
                "preset.update: cannot resolve workspace: {error}"
            ));
        }
    };
    let source: PathBuf = dx_preset::source_dir(&workspace);
    if !source.is_dir() {
        return dx_preset::bin_cannot(format!(
            "preset.update: source directory '{}' not found",
            source.display()
        ));
    }
    if verify_only {
        match dx_preset::check_preset(&workspace) {
            Ok(()) => {
                println!("preset.update: preset.bazelrc verified");
                0
            }
            Err(dx_preset::PresetError::Stale { detail }) => {
                println!("{detail}");
                1
            }
            Err(error) => dx_preset::bin_cannot(error),
        }
    } else {
        match dx_preset::update_preset(&workspace) {
            Ok(()) => {
                println!("preset.update: wrote preset.bazelrc");
                0
            }
            Err(error) => dx_preset::bin_cannot(error),
        }
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
