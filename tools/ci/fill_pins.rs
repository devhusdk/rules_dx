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
    repo: &str,
    verify: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<String, String> {
    check_tag(tag, pins_bzl)?;
    let manifest = dir.join(MANIFEST);
    let bundle = dir.join(BUNDLE);
    for file in [&manifest, &bundle] {
        if !file.is_file() {
            return Err(format!("{} is missing", file.display()));
        }
    }
    verify(&manifest, &bundle)?;
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("read {MANIFEST}: {e}"))?;
    fill_from_manifest(tag, &text, pins_bzl, repo)
}

/// Fills `pins_bzl` from manifest text that the caller has already trusted.
/// The release URL points at `repo`, written `OWNER/NAME`.
pub fn fill_from_manifest(
    tag: &str,
    manifest_text: &str,
    pins_bzl: &str,
    repo: &str,
) -> Result<String, String> {
    check_tag(tag, pins_bzl)?;
    check_repo(repo)?;
    let pins = LauncherPins::from_sha256sums(manifest_text).map_err(|e| e.to_string())?;
    rewrite_repo(&rewrite(pins_bzl, &pins)?, repo)
}

fn check_tag(tag: &str, pins_bzl: &str) -> Result<(), String> {
    let version = declared_version(pins_bzl)?;
    if tag != format!("v{version}") {
        return Err(format!("tag {tag} differs from DX_VERSION {version}"));
    }
    Ok(())
}

fn check_repo(repo: &str) -> Result<(), String> {
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match repo.split_once('/') {
        Some((owner, name)) if valid(owner) && valid(name) => Ok(()),
        _ => Err(format!("repo {repo:?} must be OWNER/NAME")),
    }
}

fn rewrite_repo(pins_bzl: &str, repo: &str) -> Result<String, String> {
    let mut found = false;
    let mut out = String::new();
    for line in pins_bzl.split_inclusive('\n') {
        if line.starts_with("URL_TEMPLATE = ") {
            found = true;
            out.push_str(&line.replace("devhusdk/rules_dx", repo));
        } else {
            out.push_str(line);
        }
    }
    if !found {
        return Err("pins.bzl has no URL_TEMPLATE".to_owned());
    }
    Ok(out)
}

/// Passes when `pins_bzl` is all empty, or when its hashes equal the verified manifest in `dir`.
/// With `filled`, all-empty pins fail.
/// `fetch` runs only when there are pins to compare.
pub fn check(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    repo: &str,
    filled: bool,
    fetch: impl FnOnce() -> Result<(), String>,
    verify: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let values = current_values(pins_bzl)?;
    let empty = values.iter().filter(|value| value.is_empty()).count();
    if empty == values.len() {
        if filled {
            return Err("pins.bzl has empty pins".to_owned());
        }
        return Ok(());
    }
    if empty != 0 {
        return Err("pins.bzl mixes empty and filled pins".to_owned());
    }
    fetch()?;
    let expected = fill(tag, dir, pins_bzl, repo, verify)?;
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
