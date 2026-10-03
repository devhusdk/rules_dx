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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One runfiles manifest read into key and path pairs.
pub fn read_manifest(manifest: &Path) -> std::io::Result<BTreeMap<String, PathBuf>> {
    let text = std::fs::read_to_string(manifest)?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some((key, path)) = line.split_once(' ') {
            out.insert(key.to_owned(), PathBuf::from(path));
        }
    }
    Ok(out)
}

/// Returns the entry point a generated launcher script names.
pub fn entry_point(script: &Path) -> std::io::Result<String> {
    let text = std::fs::read_to_string(script)?;
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
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("no entry point in {}", script.display()),
    ))
}

/// Returns the directory a generated launcher's entry point is named from.
///
/// The entry point is written the way the execroot spells it, and the generated
/// script sits three directories below that.
pub fn bin_dir(script: &Path) -> Option<PathBuf> {
    script.ancestors().nth(3).map(Path::to_path_buf)
}

#[cfg(test)]
mod lib_tests;
