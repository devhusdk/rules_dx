//! Splits an ELF binary into a stripped binary, a debug file and a manifest.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::PathBuf;

use dx_atomic_fs::path_io;
use dx_symbols::cli::{CliError, parse_flags, run_tool, write_manifest};

const FLAGS: [&str; 5] = ["--objcopy", "--in", "--stripped", "--debug", "--manifest"];
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

fn split([objcopy, input, stripped, debug, manifest]: &[PathBuf; 5]) -> Result<(), CliError> {
    if !input.is_file() {
        return Err(CliError::NoInput(input.clone()));
    }
    run_tool(
        objcopy,
        &["--only-keep-debug".as_ref(), input.as_ref(), debug.as_ref()],
        TOOL_TIMEOUT,
    )?;
    let link = format!(
        "--add-gnu-debuglink={}",
        debug
            .to_str()
            .ok_or_else(|| CliError::NotUtf8(debug.clone()))?
    );
    run_tool(
        objcopy,
        &[
            "--strip-all".as_ref(),
            link.as_ref(),
            input.as_ref(),
            stripped.as_ref(),
        ],
        TOOL_TIMEOUT,
    )?;
    let stripped_bytes = path_io::read(stripped)?;
    let debug_bytes = path_io::read(debug)?;
    let pair = dx_symbols::check_pair(&stripped_bytes, &debug_bytes)?;
    let version = run_tool(objcopy, &["--version".as_ref()], TOOL_TIMEOUT)?.stdout_text();
    let json = serde_json::json!({
        "build_id": pair.build_id,
        "arch": pair.arch,
        "sha256": dx_digest::sha256_hex(&stripped_bytes),
        "debug_file": debug.file_name().and_then(|n| n.to_str()),
        "objcopy": version.lines().next().unwrap_or(""),
    });
    write_manifest(manifest, &json)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = match parse_flags(&args, FLAGS) {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("split: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = split(&paths) {
        let [_, _, stripped, debug, manifest] = &paths;
        for p in [stripped, debug, manifest] {
            dx_atomic_fs::discard_file(p);
        }
        eprintln!("split: {e}");
        std::process::exit(1);
    }
}
