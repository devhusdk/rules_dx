use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_atomic_fs::path_io;

use crate::cli::{CliError, parse_flags, run_tool, write_manifest};

const SPLIT_TIMEOUT: Duration = Duration::from_secs(600);
const DSYM_TIMEOUT: Duration = Duration::from_secs(120);

#[must_use]
pub fn split(argv: &[String]) -> i32 {
    let args = argv.get(1..).unwrap_or_default();
    let paths = match parse_flags(
        args,
        ["--objcopy", "--in", "--stripped", "--debug", "--manifest"],
    ) {
        Ok(paths) => paths,
        Err(e) => return fail("split", &e),
    };
    match split_paths(&paths) {
        Ok(()) => 0,
        Err(e) => {
            let [_, _, stripped, debug, manifest] = &paths;
            for p in [stripped, debug, manifest] {
                dx_atomic_fs::discard_file(p);
            }
            fail("split", &e)
        }
    }
}

#[must_use]
pub fn dsym(argv: &[String]) -> i32 {
    let args = argv.get(1..).unwrap_or_default();
    match dsym_args(args) {
        Ok(()) => 0,
        Err(e) => fail("dsym", &e),
    }
}

#[must_use]
pub fn pdb(argv: &[String]) -> i32 {
    let args = argv.get(1..).unwrap_or_default();
    match pdb_args(args) {
        Ok(()) => 0,
        Err(e) => fail("pdb", &e),
    }
}

fn fail(tool: &str, error: &CliError) -> i32 {
    eprintln!("{tool}: {error}");
    1
}

fn split_paths([objcopy, input, stripped, debug, manifest]: &[PathBuf; 5]) -> Result<(), CliError> {
    if !input.is_file() {
        return Err(CliError::NoInput(input.clone()));
    }
    run_tool(
        objcopy,
        &["--only-keep-debug".as_ref(), input.as_ref(), debug.as_ref()],
        SPLIT_TIMEOUT,
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
        SPLIT_TIMEOUT,
    )?;
    let stripped_bytes = path_io::read(stripped)?;
    let debug_bytes = path_io::read(debug)?;
    let pair = crate::check_pair(&stripped_bytes, &debug_bytes)?;
    let version = run_tool(objcopy, &["--version".as_ref()], SPLIT_TIMEOUT)?.stdout_text();
    let json = serde_json::json!({
        "build_id": pair.build_id,
        "arch": pair.arch,
        "sha256": dx_digest::sha256_hex(&stripped_bytes),
        "debug_file": debug.file_name().and_then(|n| n.to_str()),
        "objcopy": version.lines().next().unwrap_or(""),
    });
    write_manifest(manifest, &json)
}

fn dsymutil_version(dsymutil: &Path) -> Result<String, CliError> {
    let text = run_tool(dsymutil, &["--version".as_ref()], DSYM_TIMEOUT)?.stdout_text();
    Ok(text
        .lines()
        .find(|l| l.contains("version"))
        .unwrap_or("")
        .trim()
        .to_string())
}

fn dsym_args(args: &[String]) -> Result<(), CliError> {
    let [dsymutil, input, bundle, manifest] =
        parse_flags(args, ["--dsymutil", "--in", "--bundle", "--manifest"])?;
    let tool = dsymutil_version(&dsymutil)?;
    write_manifest(&manifest, &crate::dsym_manifest(&input, &bundle, &tool)?)
}

fn pdb_args(args: &[String]) -> Result<(), CliError> {
    let [input, pdb, manifest] = parse_flags(args, ["--in", "--pdb", "--manifest"])?;
    write_manifest(&manifest, &crate::pdb_manifest(&input, &pdb)?)
}
