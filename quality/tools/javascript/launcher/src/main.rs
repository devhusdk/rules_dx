//! Runs a generated JavaScript tool through the node runtime beside it.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use dx_js_launcher::{bin_dir, entry_point, read_manifest};

/// The runfiles key suffix naming the file that says which tool to run.
const DESCRIPTOR_SUFFIX: &str = ".launcher.txt";

fn main() -> std::process::ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("js_launcher: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> std::io::Result<std::process::ExitCode> {
    let argv: Vec<String> = std::env::args().collect();
    let exe = argv
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| bad("no argv[0]"))?;
    let manifest_path = dx_path::manifest_for(&exe);
    let manifest = read_manifest(&manifest_path)?;

    let (script_key, node_key) = descriptor(&manifest)?;
    let script = resolve(&manifest, &script_key)?;
    let node = resolve(&manifest, &node_key)?;
    let base = bin_dir(&script).ok_or_else(|| bad("cannot place the entry point"))?;
    let entry = base.join(entry_point(&script)?);

    let status = Command::new(&node)
        .arg(&entry)
        .args(argv.iter().skip(1))
        .status()?;
    Ok(std::process::ExitCode::from(
        status.code().unwrap_or(1) as u8
    ))
}

/// Returns the script and node runfiles keys the descriptor names.
fn descriptor(
    manifest: &std::collections::BTreeMap<String, PathBuf>,
) -> std::io::Result<(String, String)> {
    let key = manifest
        .keys()
        .find(|key| key.ends_with(DESCRIPTOR_SUFFIX))
        .ok_or_else(|| bad("no launcher descriptor in the runfiles"))?
        .clone();
    let path = manifest[&key].clone();
    let text = std::fs::read_to_string(&path)?;
    let mut script = None;
    let mut node = None;
    for line in text.lines() {
        if let Some((name, value)) = line.split_once('=') {
            match name.trim() {
                "script" => script = Some(value.trim().to_owned()),
                "node" => node = Some(value.trim().to_owned()),
                _ => {}
            }
        }
    }
    match (script, node) {
        (Some(script), Some(node)) => Ok((script, node)),
        _ => Err(bad(format!("incomplete descriptor in {}", path.display()))),
    }
}

/// Returns the runfiles path one key names.
fn resolve(
    manifest: &std::collections::BTreeMap<String, PathBuf>,
    key: &str,
) -> std::io::Result<PathBuf> {
    manifest
        .get(key)
        .cloned()
        .ok_or_else(|| bad(format!("{key} is not in the runfiles")))
}

fn bad(message: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.to_string())
}

/// Keeps the unused import warning away on hosts that never resolve a path.
#[allow(dead_code)]
fn unused(_: &Path) {}
