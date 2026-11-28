#![deny(clippy::indexing_slicing, clippy::string_slice)]

const SUBCOMMAND: &str = "haskell_extract";
const EXIT_USAGE: i32 = 2;

// LCOV_EXCL_START - reason: thin subcommand shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_none_or(|arg| arg != SUBCOMMAND) {
        eprintln!("{SUBCOMMAND}: the first argument must be {SUBCOMMAND}");
        std::process::exit(EXIT_USAGE);
    }
    let args: Vec<String> = args.map(|arg| arg.to_string_lossy().into_owned()).collect();
    std::process::exit(dx_docs_haskell_extract::run_process(&args));
}
// LCOV_EXCL_STOP - reason: end thin subcommand shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
