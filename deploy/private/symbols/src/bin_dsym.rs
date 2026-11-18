//! Checks a macOS dSYM bundle against its binary and writes the dsym manifest.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::Path;

use dx_symbols::cli::{CliError, parse_flags, run_tool, write_manifest};

const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn version(dsymutil: &Path) -> Result<String, CliError> {
    let text = run_tool(dsymutil, &["--version".as_ref()], TOOL_TIMEOUT)?.stdout_text();
    Ok(text
        .lines()
        .find(|l| l.contains("version"))
        .unwrap_or("")
        .trim()
        .to_string())
}

fn run(args: &[String]) -> Result<(), CliError> {
    let [dsymutil, input, bundle, manifest] =
        parse_flags(args, ["--dsymutil", "--in", "--bundle", "--manifest"])?;
    let tool = version(&dsymutil)?;
    write_manifest(
        &manifest,
        &dx_symbols::dsym_manifest(&input, &bundle, &tool)?,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("dsym: {e}");
        std::process::exit(1);
    }
}
