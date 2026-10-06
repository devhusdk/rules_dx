//! Reads a generated JavaScript launcher's own runfiles so a host can run the tool.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io;
use std::path::{Path, PathBuf};

/// The program a launcher runs, with the entry point it runs.
#[derive(Debug)]
pub struct Plan {
    pub program: PathBuf,
    pub entry: PathBuf,
    pub args: Vec<String>,
}

/// Names the descriptor beside one launcher.
pub fn descriptor_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_stem().unwrap_or(exe.as_os_str()).to_os_string();
    name.push(".launcher.txt");
    exe.with_file_name(name)
}

/// Returns the entry point a generated launcher script names.
pub fn entry_point(script: &Path) -> io::Result<String> {
    let text = read(script)?;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("entry_point=$") {
            continue;
        }
        if let Some((_, quoted)) = trimmed.split_once('"') {
            let (path, _) = quoted.split_once('"').unwrap_or((quoted, ""));
            if !path.is_empty() {
                return Ok(path.to_owned());
            }
        }
    }
    Err(bad(format!("no entry point in {}", script.display())))
}

/// Returns the directory a generated launcher's entry point is named from.
///
/// The entry point is written the way the execroot spells it, and the generated
/// script sits three directories below that.
pub fn bin_dir(script: &Path) -> Option<PathBuf> {
    script.ancestors().nth(3).map(Path::to_path_buf)
}

/// Returns the script and node runfiles keys one descriptor names.
pub fn descriptor(path: &Path) -> io::Result<(String, String)> {
    let text = read(path)?;
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

/// Resolves what one launcher invocation runs from its own runfiles.
pub fn plan(exe: &Path, args: &[String]) -> io::Result<Plan> {
    let (script_key, node_key) = descriptor(&descriptor_path(exe))?;
    let runfiles = dx_path::Resolver::for_binary(exe)?;
    let script = runfiles.lookup(&script_key)?;
    let node = runfiles.lookup(&node_key)?;
    let base = bin_dir(&script).ok_or_else(|| {
        bad(format!(
            "cannot place the entry point of {}",
            script.display()
        ))
    })?;
    Ok(Plan {
        program: node,
        entry: base.join(entry_point(&script)?),
        args: args.to_vec(),
    })
}

fn read(path: &Path) -> io::Result<String> {
    std::fs::read_to_string(path)
        .map_err(|error| bad(format!("cannot read {}: {error}", path.display())))
}

fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}

#[cfg(test)]
mod lib_tests;
