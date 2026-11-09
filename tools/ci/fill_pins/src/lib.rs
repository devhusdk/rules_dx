//! Rewrites the `SHA256` and `GAZELLE_SHA256` values in `cli/prebuilt/pins.bzl`
//! from a verified `dx-sha256sums.txt` and `dx-gazelle-sha256sums.txt`.

use dx_adopt::{LauncherPins, RELEASE_ASSETS};
use std::collections::BTreeMap;
use std::path::Path;

pub const MANIFEST: &str = "dx-sha256sums.txt";
pub const GAZELLE_MANIFEST: &str = "dx-gazelle-sha256sums.txt";
pub const BUNDLE: &str = "dx-sha256sums.txt.sigstore.json";

/// The gazelle release asset beside each dx release asset.
pub fn gazelle_assets() -> Vec<String> {
    RELEASE_ASSETS
        .iter()
        .map(|asset| format!("dx-gazelle-{}", asset.trim_start_matches("dx-")))
        .collect()
}

/// Reads both manifests from `dir`, checks each with `verify`, and returns the new `pins.bzl` text.
pub fn fill(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    repo: &str,
    verify: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<String, String> {
    check_tag(tag, pins_bzl)?;
    let manifest = dir.join(MANIFEST);
    let gazelle = dir.join(GAZELLE_MANIFEST);
    let bundle = dir.join(BUNDLE);
    for file in [&manifest, &gazelle, &bundle] {
        if !file.is_file() {
            return Err(format!("{} is missing", file.display()));
        }
    }
    verify(&manifest, &bundle)?;
    verify(&gazelle, &bundle)?;
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
    };
    fill_from_manifest(tag, &read(&manifest)?, &read(&gazelle)?, pins_bzl, repo)
}

/// Fills `pins_bzl` from manifest texts that the caller has already trusted.
/// The release URL points at `repo`, written `OWNER/NAME`.
pub fn fill_from_manifest(
    tag: &str,
    manifest_text: &str,
    gazelle_text: &str,
    pins_bzl: &str,
    repo: &str,
) -> Result<String, String> {
    check_tag(tag, pins_bzl)?;
    check_repo(repo)?;
    let pins = LauncherPins::from_sha256sums(manifest_text).map_err(|e| e.to_string())?;
    let mut hashes = parse_gazelle_manifest(gazelle_text)?;
    for asset in RELEASE_ASSETS {
        let hash = pins
            .get(asset)
            .ok_or_else(|| format!("{asset} has no pin"))?;
        hashes.insert(asset.to_owned(), hash.to_owned());
    }
    rewrite_repo(&rewrite(pins_bzl, &hashes)?, repo)
}

/// Parses `dx-gazelle-sha256sums.txt`: exactly one `<hash>  <asset>` line per
/// gazelle asset, hash as 64 lowercase hex, nothing else.
pub fn parse_gazelle_manifest(text: &str) -> Result<BTreeMap<String, String>, String> {
    let assets = gazelle_assets();
    let mut hashes = BTreeMap::new();
    for line in text.lines() {
        let Some((hash, asset)) = line.split_once("  ") else {
            return Err(format!("{GAZELLE_MANIFEST}: bad line {line:?}"));
        };
        if !assets.iter().any(|known| known == asset) {
            return Err(format!("{GAZELLE_MANIFEST}: unknown asset {asset:?}"));
        }
        if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(format!(
                "{GAZELLE_MANIFEST}: {asset} hash is not 64 lowercase hex"
            ));
        }
        if hashes.insert(asset.to_owned(), hash.to_owned()).is_some() {
            return Err(format!("{GAZELLE_MANIFEST}: {asset} is listed twice"));
        }
    }
    if let Some(missing) = assets.iter().find(|asset| !hashes.contains_key(*asset)) {
        return Err(format!("{GAZELLE_MANIFEST}: {missing} is missing"));
    }
    Ok(hashes)
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

/// Passes when `pins_bzl` is all empty, or when its hashes equal the verified manifests in `dir`.
/// With `filled`, all-empty pins fail.
/// `fetch` runs only when there are pins to compare.
pub fn check(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    repo: &str,
    filled: bool,
    fetch: impl FnOnce() -> Result<(), String>,
    verify: impl Fn(&Path, &Path) -> Result<(), String>,
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
            "pins.bzl differs from the attested {MANIFEST} and {GAZELLE_MANIFEST} of {tag}"
        ));
    }
    Ok(())
}

fn all_assets() -> Vec<String> {
    let mut assets: Vec<String> = RELEASE_ASSETS.iter().map(|a| (*a).to_owned()).collect();
    assets.extend(gazelle_assets());
    assets
}

fn current_values(pins_bzl: &str) -> Result<Vec<String>, String> {
    all_assets()
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

fn rewrite(pins_bzl: &str, hashes: &BTreeMap<String, String>) -> Result<String, String> {
    let assets = all_assets();
    let mut out = String::new();
    let mut replaced = 0;
    for line in pins_bzl.split_inclusive('\n') {
        let asset = assets
            .iter()
            .find(|asset| line.starts_with(&format!("    \"{asset}\": \"")));
        match asset {
            Some(asset) => {
                let hash = hashes
                    .get(asset)
                    .ok_or_else(|| format!("{asset} has no pin"))?;
                let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
                out.push_str(&format!("    \"{asset}\": \"{hash}\",{ending}"));
                replaced += 1;
            }
            None => out.push_str(line),
        }
    }
    if replaced != assets.len() {
        return Err(format!(
            "pins.bzl lists {replaced} of {} assets",
            assets.len()
        ));
    }
    Ok(out)
}
