//! Rewrites the `SHA256` values in `cli/prebuilt/pins.bzl` from a verified `dx-sha256sums.txt`.

use dx_adopt::{LauncherPins, RELEASE_ASSETS};
use std::path::Path;

pub const MANIFEST: &str = "dx-sha256sums.txt";
pub const BUNDLE: &str = "dx-sha256sums.txt.sigstore.json";

/// Reads the manifest from `dir`, checks it with `verify`, and returns the new `pins.bzl` text.
pub fn fill(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    verify: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<String, String> {
    let version = declared_version(pins_bzl)?;
    if tag != format!("v{version}") {
        return Err(format!("tag {tag} differs from DX_VERSION {version}"));
    }
    let manifest = dir.join(MANIFEST);
    let bundle = dir.join(BUNDLE);
    for file in [&manifest, &bundle] {
        if !file.is_file() {
            return Err(format!("{} is missing", file.display()));
        }
    }
    verify(&manifest, &bundle)?;
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("read {MANIFEST}: {e}"))?;
    let pins = LauncherPins::from_sha256sums(&text).map_err(|e| e.to_string())?;
    rewrite(pins_bzl, &pins)
}

/// Passes when `pins_bzl` is all empty, or when its hashes equal the verified manifest in `dir`.
/// `fetch` runs only when there are pins to compare.
pub fn check(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    fetch: impl FnOnce() -> Result<(), String>,
    verify: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let values = current_values(pins_bzl)?;
    let empty = values.iter().filter(|value| value.is_empty()).count();
    if empty == values.len() {
        return Ok(());
    }
    if empty != 0 {
        return Err("pins.bzl mixes empty and filled pins".to_owned());
    }
    fetch()?;
    let expected = fill(tag, dir, pins_bzl, verify)?;
    if expected != pins_bzl {
        return Err(format!(
            "pins.bzl differs from the attested {MANIFEST} of {tag}"
        ));
    }
    Ok(())
}

fn current_values(pins_bzl: &str) -> Result<Vec<String>, String> {
    RELEASE_ASSETS
        .iter()
        .map(|asset| {
            let prefix = format!("    \"{asset}\": \"");
            pins_bzl
                .lines()
                .find_map(|line| line.strip_prefix(&prefix))
                .and_then(|rest| rest.strip_suffix("\","))
                .map(str::to_owned)
                .ok_or_else(|| format!("pins.bzl has no pin line for {asset}"))
        })
        .collect()
}

fn declared_version(pins_bzl: &str) -> Result<String, String> {
    pins_bzl
        .lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| "pins.bzl has no DX_VERSION".to_owned())
}

fn rewrite(pins_bzl: &str, pins: &LauncherPins) -> Result<String, String> {
    let mut out = String::new();
    let mut replaced = 0;
    for line in pins_bzl.split_inclusive('\n') {
        let asset = RELEASE_ASSETS
            .iter()
            .find(|asset| line.starts_with(&format!("    \"{asset}\": \"")));
        match asset {
            Some(asset) => {
                let hash = pins
                    .get(asset)
                    .ok_or_else(|| format!("{asset} has no pin"))?;
                let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
                out.push_str(&format!("    \"{asset}\": \"{hash}\",{ending}"));
                replaced += 1;
            }
            None => out.push_str(line),
        }
    }
    if replaced != RELEASE_ASSETS.len() {
        return Err(format!(
            "pins.bzl lists {replaced} of {} assets",
            RELEASE_ASSETS.len()
        ));
    }
    Ok(out)
}
