#![deny(clippy::indexing_slicing, clippy::string_slice)]

const SUBCOMMAND: &str = "ocaml_dune";
const EXIT_USAGE: i32 = 2;

// LCOV_EXCL_START - reason: thin subcommand shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    if ocaml_cc_driver::is_driver(&argv0) {
        std::process::exit(ocaml_cc_driver::run_tool(&argv0, args.collect()));
    }
    if args.next().is_none_or(|arg| arg != SUBCOMMAND) {
        eprintln!("{SUBCOMMAND}: the first argument must be {SUBCOMMAND}");
        std::process::exit(EXIT_USAGE);
    }
    let args: Vec<String> = args.map(|arg| arg.to_string_lossy().into_owned()).collect();
    std::process::exit(ocaml_cc_driver::run_dune(&args));
}
// LCOV_EXCL_STOP - reason: end thin subcommand shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
