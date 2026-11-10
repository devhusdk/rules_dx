//! Splits an ELF binary into a stripped binary, a debug file and a manifest.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

struct Args {
    objcopy: PathBuf,
    input: PathBuf,
    stripped: PathBuf,
    debug: PathBuf,
    manifest: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut found: [Option<PathBuf>; 5] = Default::default();
    let names = ["--objcopy", "--in", "--stripped", "--debug", "--manifest"];
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let slot = names
            .iter()
            .position(|n| n == flag)
            .ok_or_else(|| format!("unknown flag {flag}"))?;
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        found[slot] = Some(PathBuf::from(value));
    }
    let mut take = |i: usize| {
        found[i]
            .take()
            .ok_or_else(|| format!("{} is required", names[i]))
    };
    Ok(Args {
        objcopy: take(0)?,
        input: take(1)?,
        stripped: take(2)?,
        debug: take(3)?,
        manifest: take(4)?,
    })
}

const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

fn run(objcopy: &Path, args: &[&std::ffi::OsStr]) -> Result<String, String> {
    let mut command = Command::new(objcopy);
    command.args(args);
    let out = dx_process::capture::run_captured(&mut command, TOOL_TIMEOUT)
        .map_err(|e| format!("cannot run {}: {e}", objcopy.display()))?;
    if !out.success() {
        return Err(format!(
            "{} failed: {}",
            objcopy.display(),
            out.stderr_text()
        ));
    }
    Ok(out.stdout_text())
}

fn split(a: &Args) -> Result<(), String> {
    if !a.input.is_file() {
        return Err(format!("{}: no such file", a.input.display()));
    }
    run(
        &a.objcopy,
        &[
            "--only-keep-debug".as_ref(),
            a.input.as_ref(),
            a.debug.as_ref(),
        ],
    )?;
    let link = format!(
        "--add-gnu-debuglink={}",
        a.debug.to_str().ok_or("debug path is not UTF-8")?
    );
    run(
        &a.objcopy,
        &[
            "--strip-all".as_ref(),
            link.as_ref(),
            a.input.as_ref(),
            a.stripped.as_ref(),
        ],
    )?;
    let stripped = std::fs::read(&a.stripped).map_err(|e| e.to_string())?;
    let debug = std::fs::read(&a.debug).map_err(|e| e.to_string())?;
    let pair = dx_symbols::check_pair(&stripped, &debug)?;
    let version = run(&a.objcopy, &["--version".as_ref()])?;
    let manifest = serde_json::json!({
        "build_id": pair.build_id,
        "arch": pair.arch,
        "sha256": format!("{:x}", Sha256::digest(&stripped)),
        "debug_file": a.debug.file_name().and_then(|n| n.to_str()),
        "objcopy": version.lines().next().unwrap_or(""),
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&a.manifest, text + "\n").map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = parse(&args).and_then(|a| split(&a));
    if let Err(e) = result {
        if let Ok(a) = parse(&args) {
            for p in [&a.stripped, &a.debug, &a.manifest] {
                let _ = std::fs::remove_file(p);
            }
        }
        eprintln!("split: {e}");
        std::process::exit(1);
    }
}
