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
    let args = match dx_install_tools::parse_args(argv) {
        Ok(args) => args,
        Err(error) => {
            if error.0.starts_with("usage:") {
                println!("{error}");
                return 0;
            }
            eprintln!("{error}");
            return 1;
        }
    };
    match dx_install_tools::verify(&args, &dx_install_tools::SystemVerifier) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            1
        }
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
