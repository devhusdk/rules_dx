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
    if argv.len() != 6 {
        return dx_release_tools::bin_usage(
            "cosign_fetch_check",
            "<binary> <checksums> <curl-version> <version> <sha256>",
        );
    }
    let binary = match std::fs::read(&argv[1]) {
        Ok(bytes) => bytes,
        Err(error) => {
            return dx_release_tools::bin_error(format!(
                "cosign_fetch_check: cannot read {}: {error}",
                argv[1]
            ));
        }
    };
    let checksums = match std::fs::read_to_string(&argv[2]) {
        Ok(text) => text,
        Err(error) => {
            return dx_release_tools::bin_error(format!(
                "cosign_fetch_check: cannot read {}: {error}",
                argv[2]
            ));
        }
    };
    let curl_version = match std::fs::read_to_string(&argv[3]) {
        Ok(text) => text,
        Err(error) => {
            return dx_release_tools::bin_error(format!(
                "cosign_fetch_check: cannot read {}: {error}",
                argv[3]
            ));
        }
    };
    match dx_release_tools::ghcr::check_cosign_fetch(
        &binary,
        &checksums,
        &curl_version,
        &argv[4],
        &argv[5],
    ) {
        Ok(report) => {
            println!("curl version gate ok: {}", report.curl_version_line);
            println!(
                "cosign sha256 ok: {} (pin {}, checksums entry matches)",
                report.digest, argv[4]
            );
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
