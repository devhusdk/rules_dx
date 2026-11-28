#![deny(clippy::indexing_slicing, clippy::string_slice)]

const SUBCOMMAND: &str = "appimage_packer";
const EXIT_USAGE: i32 = 2;

// LCOV_EXCL_START - reason: thin subcommand shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    if std::path::Path::new(&argv0).file_stem()
        == Some(std::ffi::OsStr::new(appimage_packer::FILE_COMMAND))
    {
        std::process::exit(i32::from(appimage_packer::refuse_file()));
    }
    if args.next().is_none_or(|arg| arg != SUBCOMMAND) {
        eprintln!("{SUBCOMMAND}: the first argument must be {SUBCOMMAND}");
        std::process::exit(EXIT_USAGE);
    }
    let args: Vec<String> = args.map(|arg| arg.to_string_lossy().into_owned()).collect();
    std::process::exit(i32::from(appimage_packer::run(&args)));
}
// LCOV_EXCL_STOP - reason: end thin subcommand shim, issue: 6517, policy: docs/cli/commands/build-test-coverage.md
