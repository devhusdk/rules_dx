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

use dx_release_tools::dry_run::{parse_approve, DryRunInputs};

const USAGE: &str = "<--stage DIR> <--approve true|false> <--binary FILE> <--standalone FILE> <--standalone-checksum FILE> <--spdx FILE> <--provenance FILE> <--notice FILE> <--module-bazel FILE> <--install-lib FILE> [--summary FILE]";

fn parse(args: &[String]) -> Result<Option<DryRunInputs>, String> {
    if args.is_empty() {
        return Ok(None);
    }
    let mut stage = None;
    let mut approve = None;
    let mut binary = None;
    let mut standalone = None;
    let mut standalone_checksum = None;
    let mut spdx = None;
    let mut provenance = None;
    let mut notice = None;
    let mut module_bazel = None;
    let mut install_lib = None;
    let mut summary = None;
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("publish-dry-run: {flag} wants a value"))?;
        match flag.as_str() {
            "--stage" => stage = Some(PathBuf::from(value)),
            "--approve" => approve = Some(parse_approve(value)?),
            "--binary" => binary = Some(PathBuf::from(value)),
            "--standalone" => standalone = Some(PathBuf::from(value)),
            "--standalone-checksum" => standalone_checksum = Some(PathBuf::from(value)),
            "--spdx" => spdx = Some(PathBuf::from(value)),
            "--provenance" => provenance = Some(PathBuf::from(value)),
            "--notice" => notice = Some(PathBuf::from(value)),
            "--module-bazel" => module_bazel = Some(PathBuf::from(value)),
            "--install-lib" => install_lib = Some(PathBuf::from(value)),
            "--summary" => summary = Some(PathBuf::from(value)),
            other => return Err(format!("publish-dry-run: unknown flag {other}")),
        }
    }
    Ok(Some(DryRunInputs {
        stage: stage.ok_or("publish-dry-run: missing --stage")?,
        approve: approve.ok_or("publish-dry-run: missing --approve")?,
        binary: binary.ok_or("publish-dry-run: missing --binary")?,
        standalone: standalone.ok_or("publish-dry-run: missing --standalone")?,
        standalone_checksum: standalone_checksum
            .ok_or("publish-dry-run: missing --standalone-checksum")?,
        spdx: spdx.ok_or("publish-dry-run: missing --spdx")?,
        provenance: provenance.ok_or("publish-dry-run: missing --provenance")?,
        notice: notice.ok_or("publish-dry-run: missing --notice")?,
        module_bazel: module_bazel.ok_or("publish-dry-run: missing --module-bazel")?,
        install_lib: install_lib.ok_or("publish-dry-run: missing --install-lib")?,
        summary,
    }))
}

fn run(argv: &[String]) -> i32 {
    let inputs = match parse(&argv[1..]) {
        Ok(Some(inputs)) => inputs,
        Ok(None) => return dx_release_tools::bin_usage("publish_dry_run", USAGE),
        Err(diagnostic) => return dx_release_tools::bin_error(diagnostic),
    };
    match dx_release_tools::dry_run::run(&inputs) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
