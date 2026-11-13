#![deny(clippy::indexing_slicing, clippy::string_slice)]

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
use std::path::Path;

fn run(argv: &[String]) -> i32 {
    if let [_, flag, rest @ ..] = argv
        && flag == "--members"
    {
        let Some((dst, member_argv)) = rest.split_first().filter(|(_, args)| !args.is_empty())
        else {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--members <out> [--executable <member>]... <name=path>...",
            );
        };
        let dst = Path::new(dst);
        let parsed = match dx_deploy_tools::member_args(member_argv) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("archiver: {error}");
                return 1;
            }
        };
        let members: Vec<(&str, &Path, bool)> = parsed
            .iter()
            .map(|(name, path, executable)| (name.as_str(), path.as_path(), *executable))
            .collect();
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_members(&members, dst) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", dst, error),
        };
    }
    if let [_, flag, rest @ ..] = argv
        && flag == "--imports"
    {
        // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let Some((dst, import_argv)) = rest.split_first().filter(|(_, args)| !args.is_empty())
        else {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--imports <out> [--check <member>]... [--provides <name>]... <member=path>...",
            );
        };
        let dst = Path::new(dst);
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::check_imports(import_argv) {
            Ok(()) => match std::fs::write(dst, b"") {
                Ok(()) => 0,
                Err(error) => dx_deploy_tools::bin_cannot("archiver", "write", dst, error),
            },
            Err(error) => {
                eprintln!("archiver: {error}");
                1
            }
        };
    }
    if let [_, flag, ..] = argv
        && flag == "--oci-layout"
    {
        // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let [_, _, dst, platform, reference, image] = argv else {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--oci-layout <out> <platform> <ref> <image>",
            );
        };
        let dst = Path::new(dst);
        let image = Path::new(image);
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_oci_layout(image, platform, reference, dst) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", image, error),
        };
    }
    if let [_, flag, ..] = argv
        && flag == "--image"
    {
        // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let [_, _, dst, platform, entry, layer_argv @ ..] = argv else {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--image <out> <platform> <entrypoint> <layer>...",
            );
        };
        if layer_argv.is_empty() {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--image <out> <platform> <entrypoint> <layer>...",
            );
        }
        let dst = Path::new(dst);
        let entrypoint: Vec<&str> = entry.split_whitespace().collect();
        let layers: Vec<&Path> = layer_argv.iter().map(Path::new).collect();
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_image(&layers, platform, &entrypoint, dst) {
            Ok(_) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", dst, error),
        };
    }
    // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let [_, src, dst] = argv else {
        return dx_deploy_tools::bin_usage("archiver", "<src> <out>");
    };
    let src = Path::new(src);
    let dst = Path::new(dst);
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match dx_deploy_tools::archive_file(src, dst) {
        Ok(()) => 0,
        Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", src, error),
    }
}

fn main() {
    let mut argv: Vec<String> = std::env::args().collect();
    if argv.len() == 2
        && let Some(path) = argv.get(1).and_then(|arg| arg.strip_prefix('@'))
    {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                argv.truncate(1);
                argv.extend(text.lines().map(str::to_owned));
            }
            Err(error) => {
                eprintln!("archiver: cannot read {path}: {error}");
                std::process::exit(1);
            }
        }
    }
    std::process::exit(run(&argv));
}
