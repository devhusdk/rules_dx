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
fn run(argv: &[String]) -> i32 {
    if argv.len() != 2 {
        return dx_release_tools::bin_usage("devcontainer_check", "<dockerfile>");
    }
    let text = match std::fs::read_to_string(&argv[1]) {
        Ok(text) => text,
        Err(error) => {
            return dx_release_tools::bin_error(format!(
                "devcontainer_check: cannot read {}: {error}",
                argv[1]
            ));
        }
    };
    let report = dx_release_tools::ghcr::inspect_devcontainer(&text);
    if !report.failures.is_empty() {
        return dx_release_tools::bin_error(report.failures.join("\n"));
    }
    println!("{}", dx_release_tools::ghcr::devcontainer_summary(&report));
    0
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
