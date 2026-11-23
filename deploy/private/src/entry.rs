use std::path::{Path, PathBuf};

// LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
pub fn archiver(argv: &[String]) -> i32 {
    if let [name, arg] = argv
        && let Some(path) = arg.strip_prefix('@')
    {
        return match std::fs::read_to_string(path) {
            Ok(text) => {
                let expanded: Vec<String> = std::iter::once(name.clone())
                    .chain(text.lines().map(str::to_owned))
                    .collect();
                archive_with_args(&expanded)
            }
            Err(error) => {
                eprintln!("archiver: cannot read {path}: {error}");
                1
            }
        };
    }
    archive_with_args(argv)
}
// LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

// LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn archive_with_args(argv: &[String]) -> i32 {
    if let [_, flag, rest @ ..] = argv
        && flag == "--members"
    {
        let Some((dst, member_argv)) = rest.split_first().filter(|(_, args)| !args.is_empty())
        else {
            return crate::bin_usage(
                "archiver",
                "--members <out> [--executable <member>]... <name=path>...",
            );
        };
        let dst = Path::new(dst);
        let parsed = match crate::member_args(member_argv) {
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
        // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match crate::archive_members(&members, dst) {
            Ok(()) => 0,
            Err(error) => crate::bin_cannot("archiver", "archive", dst, error),
        };
    }
    if let [_, flag, rest @ ..] = argv
        && flag == "--imports"
    {
        // LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let Some((dst, import_argv)) = rest.split_first().filter(|(_, args)| !args.is_empty())
        else {
            return crate::bin_usage(
                "archiver",
                "--imports <out> [--check <member>]... [--provides <name>]... <member=path>...",
            );
        };
        let dst = Path::new(dst);
        // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match crate::check_imports(import_argv) {
            Ok(()) => match std::fs::write(dst, b"") {
                Ok(()) => 0,
                Err(error) => crate::bin_cannot("archiver", "write", dst, error),
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
        // LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let [_, _, dst, platform, reference, image] = argv else {
            return crate::bin_usage(
                "archiver",
                "--oci-layout <out> <platform> <ref> <image>",
            );
        };
        let dst = Path::new(dst);
        let image = Path::new(image);
        // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match crate::archive_oci_layout(image, platform, reference, dst) {
            Ok(()) => 0,
            Err(error) => crate::bin_cannot("archiver", "archive", image, error),
        };
    }
    if let [_, flag, ..] = argv
        && flag == "--image"
    {
        // LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let [_, _, dst, platform, entry, layer_argv @ ..] = argv else {
            return crate::bin_usage(
                "archiver",
                "--image <out> <platform> <entrypoint> <layer>...",
            );
        };
        if layer_argv.is_empty() {
            return crate::bin_usage(
                "archiver",
                "--image <out> <platform> <entrypoint> <layer>...",
            );
        }
        let dst = Path::new(dst);
        let entrypoint: Vec<&str> = entry.split_whitespace().collect();
        let layers: Vec<&Path> = layer_argv.iter().map(Path::new).collect();
        // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match crate::archive_image(&layers, platform, &entrypoint, dst) {
            Ok(_) => 0,
            Err(error) => crate::bin_cannot("archiver", "archive", dst, error),
        };
    }
    // LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    let [_, src, dst] = argv else {
        return crate::bin_usage("archiver", "<src> <out>");
    };
    let src = Path::new(src);
    let dst = Path::new(dst);
    // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match crate::archive_file(src, dst) {
        Ok(()) => 0,
        Err(error) => crate::bin_cannot("archiver", "archive", src, error),
    }
}

// LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
pub fn hasher(argv: &[String]) -> i32 {
    let [_, src, dst] = argv else {
        return crate::bin_usage("hasher", "<src> <out>");
    };
    let src = Path::new(src);
    let dst = Path::new(dst);
    // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match crate::hash_file(src, dst) {
        Ok(()) => 0,
        Err(error) => crate::bin_cannot("hasher", "hash", src, error),
    }
}

// LCOV_EXCL_START - reason: argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
pub fn size_checker(argv: &[String]) -> i32 {
    let budget = argv.get(1).and_then(|text| text.parse::<u64>().ok());
    let (Some(budget), Some(stamp)) = (budget, argv.get(2)) else {
        return crate::bin_usage("size_checker", "<budget_bytes> <stamp> <path>...");
    };
    let paths: Vec<PathBuf> = argv.iter().skip(3).map(PathBuf::from).collect();
    // LCOV_EXCL_STOP - reason: end argv parsing, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match crate::check_files_size_budget(&paths, budget) {
        Ok(bytes) => match std::fs::write(stamp, format!("{bytes} of {budget} bytes\n")) {
            Ok(()) => 0,
            Err(error) => crate::bin_cannot(
                "size_checker",
                "write",
                Path::new(stamp),
                error,
            ),
        },
        Err(message) => {
            eprintln!("size_checker: {message}");
            1
        }
    }
}
