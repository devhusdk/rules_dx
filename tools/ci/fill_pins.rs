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
