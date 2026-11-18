//! Writes the `PINS` of `cli/prebuilt/pins.bzl` from verified release manifests:
//! one sha256 per tool and platform that the registry (`cli/prebuilt/tools.bzl`) names.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

pub const MANIFEST: &str = "dx-sha256sums.txt";
pub const GAZELLE_MANIFEST: &str = "dx-gazelle-sha256sums.txt";
pub const BUNDLE: &str = "dx-sha256sums.txt.sigstore.json";

/// The manifests a release check downloads and verifies.
pub const MANIFESTS: [&str; 2] = [MANIFEST, GAZELLE_MANIFEST];

/// `<tool> <platform key> <asset>` lines rendered from `tools.bzl` by `//cli/prebuilt:registry`.
pub const REGISTRY: &str = include_str!(env!("DX_PREBUILT_REGISTRY"));

/// Files a release publishes beside a registry asset and lists in the same
/// manifest: the launcher archives, the Windows symbols and their SBOMs.
const COMPANION_SUFFIXES: [&str; 3] = [".tar.gz", ".zip", "-pdb.zip"];

/// One release asset the registry expects a manifest line for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub tool: String,
    pub key: String,
    pub asset: String,
}

/// Parses the registry text.
///
/// # Errors
///
/// Returns `Err` when a line does not have exactly three fields.
pub fn parse_registry(text: &str) -> Result<Vec<Entry>, String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(
            |line| match line.split(' ').collect::<Vec<_>>().as_slice() {
                [tool, key, asset] => Ok(Entry {
                    tool: (*tool).to_owned(),
                    key: (*key).to_owned(),
                    asset: (*asset).to_owned(),
                }),
                _ => Err(format!("registry: bad line {line:?}")),
            },
        )
        .collect()
}

/// Reads every manifest from `dir`, checks each with `verify`, and returns the new `pins.bzl` text.
///
/// # Errors
///
/// Returns `Err` with the diagnostic when a manifest or the pins are invalid.
pub fn fill(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    repo: &str,
    verify: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<String, String> {
    check_tag(tag, pins_bzl)?;
    let bundle = dir.join(BUNDLE);
    let manifests: Vec<_> = MANIFESTS.iter().map(|name| dir.join(name)).collect();
    for file in manifests.iter().chain([&bundle]) {
        if !file.is_file() {
            return Err(format!("{} is missing", file.display()));
        }
    }
    let mut texts = Vec::new();
    for manifest in &manifests {
        verify(manifest, &bundle)?;
        texts.push((manifest.display().to_string(), read(manifest)?));
    }
    fill_from_manifests(tag, &texts, pins_bzl, repo)
}

/// Reads a manifest file.
///
/// # Errors
///
/// Returns `Err` naming the file when it cannot be read.
pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
}

/// Fills `pins_bzl` from `(name, text)` manifests that the caller has already trusted.
/// The release URL points at `repo`, written `OWNER/NAME`.
///
/// # Errors
///
/// Returns `Err` with the diagnostic when a manifest or the pins are invalid.
pub fn fill_from_manifests(
    tag: &str,
    manifests: &[(String, String)],
    pins_bzl: &str,
    repo: &str,
) -> Result<String, String> {
    fill_with_registry(&parse_registry(REGISTRY)?, tag, manifests, pins_bzl, repo)
}

/// [`fill_from_manifests`] against an explicit registry.
///
/// # Errors
///
/// Returns `Err` on a malformed line, an asset the registry does not know, an
/// asset listed twice, or a registry entry no manifest lists.
pub fn fill_with_registry(
    registry: &[Entry],
    tag: &str,
    manifests: &[(String, String)],
    pins_bzl: &str,
    repo: &str,
) -> Result<String, String> {
    check_tag(tag, pins_bzl)?;
    check_repo(repo)?;
    let known: BTreeMap<&str, &Entry> = registry.iter().map(|e| (e.asset.as_str(), e)).collect();
    let mut hashes: BTreeMap<&str, &str> = BTreeMap::new();
    for (name, text) in manifests {
        for line in text.lines() {
            let Some((hash, asset)) = line.split_once("  ") else {
                return Err(format!("{name}: bad line {line:?}"));
            };
            if !known.contains_key(asset) {
                if companion_of(asset, &known) {
                    continue;
                }
                return Err(format!(
                    "{name}: {asset:?} is not in cli/prebuilt/tools.bzl"
                ));
            }
            if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                return Err(format!("{name}: {asset} hash is not 64 lowercase hex"));
            }
            if hashes.insert(asset, hash).is_some() {
                return Err(format!("{name}: {asset} is listed twice"));
            }
        }
    }
    let mut pins: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
    for entry in registry {
        let hash = hashes
            .get(entry.asset.as_str())
            .ok_or_else(|| format!("{} has no line in the manifests", entry.asset))?;
        pins.entry(entry.tool.as_str())
            .or_default()
            .insert(entry.key.as_str(), hash);
    }
    replace_pins(&rewrite_repo(pins_bzl, repo)?, &render_pins(&pins))
}

fn companion_of(asset: &str, known: &BTreeMap<&str, &Entry>) -> bool {
    let base = asset.strip_suffix(".spdx.json").unwrap_or(asset);
    COMPANION_SUFFIXES.iter().any(|suffix| {
        base.strip_suffix(suffix)
            .is_some_and(|stem| known.contains_key(stem))
    })
}

fn render_pins(pins: &BTreeMap<&str, BTreeMap<&str, &str>>) -> String {
    let mut out = String::from("PINS = {\n");
    for (tool, by_key) in pins {
        writeln!(out, "    \"{tool}\": {{").unwrap_or_default();
        for (key, hash) in by_key {
            writeln!(out, "        \"{key}\": \"{hash}\",").unwrap_or_default();
        }
        out.push_str("    },\n");
    }
    out.push_str("}\n");
    out
}

/// Replaces everything from the `PINS = ` line on; fill_pins owns the end of the file.
fn replace_pins(pins_bzl: &str, rendered: &str) -> Result<String, String> {
    let start = pins_bzl
        .find("\nPINS = ")
        .ok_or_else(|| "pins.bzl has no PINS".to_owned())?;
    let head = pins_bzl
        .get(..=start)
        .ok_or_else(|| "pins.bzl has no PINS".to_owned())?;
    Ok(format!("{head}{rendered}"))
}

fn check_tag(tag: &str, pins_bzl: &str) -> Result<(), String> {
    if !pins_bzl.lines().any(|line| line == "SCHEMA = 1") {
        return Err("pins.bzl is not SCHEMA = 1".to_owned());
    }
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

/// Passes when `pins_bzl` has empty `PINS`, or when it equals the fill from the verified manifests in `dir`.
/// With `filled`, empty pins fail.
/// `fetch` runs only when there are pins to compare.
///
/// # Errors
///
/// Returns `Err` with the diagnostic when a manifest or the pins are invalid.
pub fn check(
    tag: &str,
    dir: &Path,
    pins_bzl: &str,
    repo: &str,
    filled: bool,
    fetch: impl FnOnce() -> Result<(), String>,
    verify: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    if pins_bzl.lines().any(|line| line == "PINS = {}") {
        if filled {
            return Err("pins.bzl has empty pins".to_owned());
        }
        return Ok(());
    }
    fetch()?;
    let expected = fill(tag, dir, pins_bzl, repo, verify)?;
    if expected != pins_bzl {
        return Err(format!(
            "pins.bzl differs from the attested {} of {tag}",
            MANIFESTS.join(" and ")
        ));
    }
    Ok(())
}

fn declared_version(pins_bzl: &str) -> Result<String, String> {
    pins_bzl
        .lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| "pins.bzl has no DX_VERSION".to_owned())
}
