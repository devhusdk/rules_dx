use dx_adopt::RELEASE_ASSETS;
use dx_test_scratch::scratch;
use std::path::Path;
use tempfile::TempDir;

/// The C++ code generators release.yml adds to the dx manifest, one asset per platform each.
const CC_TOOLS: [&str; 3] = ["capnp", "flatc", "thrift"];

const REPO: &str = "devhusdk/rules_dx";
const NATIVE_TOOLS: [&str; 2] = ["progenitor", "topiary"];
const PINS: &str = "\"\"\"Pins.\"\"\"\n\nSCHEMA = 1\n\nDX_VERSION = \"1.2.3\"\n\nURL_TEMPLATE = \"https://github.com/devhusdk/rules_dx/releases/download/v{version}/{asset}\"\n\nPINS = {}\n";

fn hash(n: usize) -> String {
    format!("{n:x}")
        .repeat(64)
        .get(..64)
        .expect("string slice within bounds")
        .to_owned()
}

fn registry() -> Vec<fill_pins::Entry> {
    fill_pins::parse_registry(fill_pins::REGISTRY).unwrap()
}

fn assets(tool: &str) -> Vec<String> {
    registry()
        .into_iter()
        .filter(|e| e.tool == tool)
        .map(|e| e.asset)
        .collect()
}

/// The dx manifest as release.yml writes it: each binary plus its launcher archive,
/// Windows symbols and SBOMs, the native launcher assets and the native build tools.
fn manifest() -> String {
    let mut lines = Vec::new();
    for (n, asset) in assets("dx").iter().enumerate() {
        let archive = if asset.contains("windows") {
            lines.push(format!("{}  {asset}-pdb.zip\n", hash(n + 6)));
            format!("{asset}.zip")
        } else {
            format!("{asset}.tar.gz")
        };
        lines.push(format!("{}  {archive}\n", hash(n + 6)));
        lines.push(format!("{}  {archive}.spdx.json\n", hash(n + 6)));
        lines.push(format!("{}  {asset}\n", hash(n + 1)));
    }
    for (n, asset) in assets("launcher").iter().enumerate() {
        lines.push(format!("{}  {asset}\n", hash(n + 16)));
    }
    for tool in NATIVE_TOOLS {
        for asset in assets(tool) {
            lines.push(format!("{}  {asset}\n", hash(4)));
        }
    }
    for (t, tool) in CC_TOOLS.iter().enumerate() {
        for (n, asset) in assets(tool).iter().enumerate() {
            lines.push(format!("{}  {asset}\n", hash(21 + 5 * t + n)));
        }
    }
    lines.concat()
}

fn jar_manifest() -> String {
    registry()
        .into_iter()
        .filter(|e| e.key == "any")
        .map(|e| format!("{}  {}\n", hash(3), e.asset))
        .collect::<Vec<String>>()
        .concat()
}

fn gazelle_manifest() -> String {
    assets("gazelle")
        .iter()
        .enumerate()
        .map(|(n, asset)| format!("{}  {asset}\n", hash(n + 11)))
        .collect::<Vec<String>>()
        .concat()
}

fn haskell_manifest() -> String {
    ["hpc_lcov", "weeder"]
        .iter()
        .flat_map(|tool| assets(tool))
        .map(|asset| format!("{}  {asset}\n", hash(4)))
        .collect::<Vec<String>>()
        .concat()
}

fn manifests(dx: &str, gazelle: &str) -> Vec<(String, String)> {
    vec![
        (fill_pins::MANIFEST.to_owned(), dx.to_owned()),
        (fill_pins::GAZELLE_MANIFEST.to_owned(), gazelle.to_owned()),
        (fill_pins::JAR_MANIFEST.to_owned(), jar_manifest()),
        (fill_pins::HASKELL_MANIFEST.to_owned(), haskell_manifest()),
    ]
}

fn release_dir(text: &str, bundle: bool) -> TempDir {
    let dir = scratch("fill_pins").unwrap();
    std::fs::write(dir.path().join(fill_pins::MANIFEST), text).unwrap();
    std::fs::write(
        dir.path().join(fill_pins::GAZELLE_MANIFEST),
        gazelle_manifest(),
    )
    .unwrap();
    std::fs::write(dir.path().join(fill_pins::JAR_MANIFEST), jar_manifest()).unwrap();
    std::fs::write(
        dir.path().join(fill_pins::HASKELL_MANIFEST),
        haskell_manifest(),
    )
    .unwrap();
    if bundle {
        std::fs::write(dir.path().join(fill_pins::BUNDLE), "{}").unwrap();
    }
    dir
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "fill_pins::fill takes a fallible verifier"
)]
fn ok(_: &Path, _: &Path) -> Result<(), String> {
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "fill_pins::check takes a fallible fetch"
)]
fn nothing() -> Result<(), String> {
    Ok(())
}

fn filled() -> String {
    let dir = release_dir(&manifest(), true);
    fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).unwrap()
}

#[test]
fn the_registry_names_every_dx_release_asset() {
    let mut want: Vec<String> = RELEASE_ASSETS.iter().map(|a| (*a).to_owned()).collect();
    want.sort();
    assert_eq!(assets("dx"), want);
    assert_eq!(assets("gazelle").len(), want.len());
    assert_eq!(assets("launcher").len(), want.len());
    for tool in CC_TOOLS {
        assert_eq!(assets(tool).len(), want.len());
    }
}

#[test]
fn writes_one_pin_per_tool_and_platform_and_keeps_the_head() {
    let out = filled();
    let head = PINS.strip_suffix("PINS = {}\n").unwrap();
    let mut by_tool = std::collections::BTreeMap::new();
    for (n, key) in [
        "linux_arm64",
        "linux_x86_64",
        "macos_arm64",
        "windows_arm64",
        "windows_x86_64",
    ]
    .iter()
    .enumerate()
    {
        let asset = format!("dx-{}", key.replacen('_', "-", 1));
        let index = assets("dx").iter().position(|a| *a == asset).unwrap();
        assert_eq!(index, n);
        by_tool
            .entry("dx".to_owned())
            .or_insert_with(Vec::new)
            .push((key.to_string(), hash(n + 1)));
        by_tool
            .entry("gazelle".to_owned())
            .or_insert_with(Vec::new)
            .push((key.to_string(), hash(n + 11)));
        by_tool
            .entry("launcher".to_owned())
            .or_insert_with(Vec::new)
            .push((key.to_string(), hash(n + 16)));
        for (t, tool) in CC_TOOLS.iter().enumerate() {
            by_tool
                .entry((*tool).to_owned())
                .or_insert_with(Vec::new)
                .push((key.to_string(), hash(21 + 5 * t + n)));
        }
    }
    for entry in registry().into_iter().filter(|e| e.key == "any") {
        by_tool.insert(entry.tool, vec![("any".to_owned(), hash(3))]);
    }
    for tool in NATIVE_TOOLS {
        let keys: Vec<(String, String)> = registry()
            .into_iter()
            .filter(|e| e.tool == tool)
            .map(|e| (e.key, hash(4)))
            .collect();
        assert_eq!(keys.len(), 4);
        by_tool.insert(tool.to_owned(), keys);
    }
    assert!(by_tool.contains_key("javac_check"));
    for entry in registry()
        .into_iter()
        .filter(|e| e.tool == "hpc_lcov" || e.tool == "weeder")
    {
        by_tool
            .entry(entry.tool)
            .or_insert_with(Vec::new)
            .push((entry.key, hash(4)));
    }
    let mut pins = String::from("PINS = {\n");
    for (tool, by_key) in by_tool {
        pins.push_str(&format!("    \"{tool}\": {{\n"));
        for (key, hash) in by_key {
            pins.push_str(&format!("        \"{key}\": \"{hash}\",\n"));
        }
        pins.push_str("    },\n");
    }
    pins.push_str("}\n");
    assert_eq!(out, format!("{head}{pins}"));
}

#[test]
fn filling_twice_changes_nothing() {
    let once = filled();
    let twice = fill_pins::fill_from_manifests(
        "v1.2.3",
        &manifests(&manifest(), &gazelle_manifest()),
        &once,
        REPO,
    )
    .unwrap();
    assert_eq!(once, twice);
}

#[test]
fn verifies_every_manifest_with_the_one_bundle() {
    let dir = release_dir(&manifest(), true);
    let seen = std::cell::RefCell::new(Vec::new());
    fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, |manifest, bundle| {
        assert!(bundle.ends_with(fill_pins::BUNDLE));
        seen.borrow_mut()
            .push(manifest.file_name().unwrap().to_string_lossy().into_owned());
        Ok(())
    })
    .unwrap();
    assert_eq!(seen.into_inner(), fill_pins::MANIFESTS);
}

#[test]
fn refuses_a_missing_manifest_or_bundle() {
    let dir = release_dir(&manifest(), true);
    std::fs::remove_file(dir.path().join(fill_pins::GAZELLE_MANIFEST)).unwrap();
    let error = fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).unwrap_err();
    assert!(error.contains(fill_pins::GAZELLE_MANIFEST), "{error}");
    let dir = release_dir(&manifest(), false);
    let error = fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).unwrap_err();
    assert!(error.contains(fill_pins::BUNDLE), "{error}");
}

#[test]
fn refuses_when_verification_fails() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, |_, _| {
        Err("not attested".to_owned())
    })
    .unwrap_err();
    assert_eq!(error, "not attested");
}

#[test]
fn refuses_an_asset_the_registry_does_not_know() {
    let dx = format!("{}{}  dx-tool-deadnix-linux-x86_64\n", manifest(), hash(3));
    let error =
        fill_pins::fill_from_manifests("v1.2.3", &manifests(&dx, &gazelle_manifest()), PINS, REPO)
            .unwrap_err();
    assert!(
        error.contains("\"dx-tool-deadnix-linux-x86_64\" is not in cli/prebuilt/tools.bzl"),
        "{error}"
    );
}

#[test]
fn refuses_a_registry_entry_with_no_line() {
    let gazelle: String = gazelle_manifest()
        .lines()
        .skip(1)
        .map(|l| format!("{l}\n"))
        .collect();
    let error =
        fill_pins::fill_from_manifests("v1.2.3", &manifests(&manifest(), &gazelle), PINS, REPO)
            .unwrap_err();
    assert!(error.contains("has no line in the manifests"), "{error}");
}

#[test]
fn refuses_an_asset_listed_twice_across_manifests() {
    let gazelle = format!(
        "{}{}",
        gazelle_manifest(),
        manifest().lines().last().unwrap()
    );
    let error =
        fill_pins::fill_from_manifests("v1.2.3", &manifests(&manifest(), &gazelle), PINS, REPO)
            .unwrap_err();
    assert!(error.contains("is listed twice"), "{error}");
}

#[test]
fn refuses_malformed_lines_and_hashes() {
    for dx in [
        format!("{}garbage\n", manifest()),
        manifest().replace(&hash(1), &"z".repeat(64)),
    ] {
        assert!(
            fill_pins::fill_from_manifests(
                "v1.2.3",
                &manifests(&dx, &gazelle_manifest()),
                PINS,
                REPO
            )
            .is_err(),
            "{dx}"
        );
    }
}

#[test]
fn refuses_a_tag_that_differs_from_dx_version_or_an_old_schema() {
    let all = manifests(&manifest(), &gazelle_manifest());
    let error = fill_pins::fill_from_manifests("v1.2.4", &all, PINS, REPO).unwrap_err();
    assert_eq!(error, "tag v1.2.4 differs from DX_VERSION 1.2.3");
    let old = PINS.replace("SCHEMA = 1\n", "");
    let error = fill_pins::fill_from_manifests("v1.2.3", &all, &old, REPO).unwrap_err();
    assert_eq!(error, "pins.bzl is not SCHEMA = 1");
}

#[test]
fn manifest_mode_points_the_url_at_the_repo() {
    let all = manifests(&manifest(), &gazelle_manifest());
    let out = fill_pins::fill_from_manifests("v1.2.3", &all, PINS, "someone/fork").unwrap();
    assert!(out.contains(
        "URL_TEMPLATE = \"https://github.com/someone/fork/releases/download/v{version}/{asset}\""
    ));
    let error = fill_pins::fill_from_manifests("v1.2.3", &all, PINS, "no-slash").unwrap_err();
    assert!(error.contains("must be OWNER/NAME"), "{error}");
}

#[test]
fn check_passes_empty_pins_without_fetching_or_verifying() {
    let dir = release_dir("", false);
    let fetch = || Err("fetched".to_owned());
    let verify = |_: &Path, _: &Path| Err("verified".to_owned());
    fill_pins::check("v1.2.3", dir.path(), PINS, REPO, false, fetch, verify).unwrap();
}

#[test]
fn check_filled_fails_on_empty_pins() {
    let dir = release_dir("", false);
    let error = fill_pins::check("v1.2.3", dir.path(), PINS, REPO, true, nothing, ok).unwrap_err();
    assert_eq!(error, "pins.bzl has empty pins");
}

#[test]
fn check_passes_when_the_pins_equal_the_manifests() {
    let dir = release_dir(&manifest(), true);
    fill_pins::check("v1.2.3", dir.path(), &filled(), REPO, true, nothing, ok).unwrap();
}

#[test]
fn check_fails_on_a_differing_pin() {
    let dir = release_dir(&manifest(), true);
    let pins = filled().replace(&hash(11), &hash(9));
    let error =
        fill_pins::check("v1.2.3", dir.path(), &pins, REPO, false, nothing, ok).unwrap_err();
    assert!(error.contains("differs from the attested"), "{error}");
}

#[test]
fn check_fails_when_verification_fails() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::check(
        "v1.2.3",
        dir.path(),
        &filled(),
        REPO,
        false,
        nothing,
        |_, _| Err("not attested".to_owned()),
    )
    .unwrap_err();
    assert_eq!(error, "not attested");
}

#[test]
fn check_uses_the_repo_in_the_expected_pins() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::check(
        "v1.2.3",
        dir.path(),
        &filled(),
        "someone/fork",
        false,
        nothing,
        ok,
    )
    .unwrap_err();
    assert!(error.contains("differs from the attested"), "{error}");
}
