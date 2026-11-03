use std::path::Path;
use tempfile::TempDir;

const PINS: &str = "\"\"\"Pins.\"\"\"\n\nDX_VERSION = \"1.2.3\"\n\nURL = \"x\"\n\nSHA256 = {\n    \"dx-linux-arm64\": \"\",\n    \"dx-linux-x86_64\": \"\",\n    \"dx-macos-arm64\": \"\",\n    \"dx-windows-arm64\": \"\",\n    \"dx-windows-x86_64\": \"\",\n}\n";

fn hash(n: usize) -> String {
    format!("{n:x}").repeat(64)[..64].to_owned()
}

fn manifest() -> String {
    let mut lines = Vec::new();
    for (n, asset) in dx_adopt::RELEASE_ASSETS.iter().enumerate() {
        let archive = if asset.contains("windows") {
            format!("{asset}.zip")
        } else {
            format!("{asset}.tar.gz")
        };
        lines.push(format!("{}  {archive}\n", hash(n + 6)));
        lines.push(format!("{}  {asset}\n", hash(n + 1)));
    }
    lines.concat()
}

fn release_dir(text: &str, bundle: bool) -> TempDir {
    let dir = dx_test_scratch::scratch("fill_pins");
    std::fs::write(dir.path().join(fill_pins::MANIFEST), text).unwrap();
    if bundle {
        std::fs::write(dir.path().join(fill_pins::BUNDLE), "{}").unwrap();
    }
    dir
}

fn ok(_: &Path, _: &Path) -> Result<(), String> {
    Ok(())
}

#[test]
fn writes_the_five_hashes_and_nothing_else() {
    let dir = release_dir(&manifest(), true);
    let out = fill_pins::fill("v1.2.3", dir.path(), PINS, ok).unwrap();
    let mut expected = PINS.to_owned();
    for (n, asset) in dx_adopt::RELEASE_ASSETS.iter().enumerate() {
        expected = expected.replace(
            &format!("\"{asset}\": \"\""),
            &format!("\"{asset}\": \"{}\"", hash(n + 1)),
        );
    }
    assert_eq!(out, expected);
}

#[test]
fn refuses_a_malformed_manifest() {
    let dir = release_dir("nonsense\n", true);
    assert!(fill_pins::fill("v1.2.3", dir.path(), PINS, ok).is_err());
}

#[test]
fn refuses_a_missing_asset_line() {
    let text: String = manifest()
        .lines()
        .skip(2)
        .map(|l| format!("{l}\n"))
        .collect();
    let dir = release_dir(&text, true);
    assert!(fill_pins::fill("v1.2.3", dir.path(), PINS, ok).is_err());
}

#[test]
fn refuses_a_missing_bundle() {
    let dir = release_dir(&manifest(), false);
    let error = fill_pins::fill("v1.2.3", dir.path(), PINS, ok).unwrap_err();
    assert!(error.contains("is missing"), "{error}");
}

#[test]
fn refuses_a_tag_that_differs_from_dx_version() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::fill("v1.2.4", dir.path(), PINS, ok).unwrap_err();
    assert!(error.contains("differs from DX_VERSION"), "{error}");
}

#[test]
fn refuses_when_verification_fails() {
    let dir = release_dir(&manifest(), true);
    let error =
        fill_pins::fill("v1.2.3", dir.path(), PINS, |_, _| Err("bad bundle".into())).unwrap_err();
    assert_eq!(error, "bad bundle");
}
