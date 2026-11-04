use std::path::Path;
use tempfile::TempDir;

const REPO: &str = "devhusdk/rules_dx";
const PINS: &str = "\"\"\"Pins.\"\"\"\n\nDX_VERSION = \"1.2.3\"\n\nURL_TEMPLATE = \"https://github.com/devhusdk/rules_dx/releases/download/v{version}/{asset}\"\n\nSHA256 = {\n    \"dx-linux-arm64\": \"\",\n    \"dx-linux-x86_64\": \"\",\n    \"dx-macos-arm64\": \"\",\n    \"dx-windows-arm64\": \"\",\n    \"dx-windows-x86_64\": \"\",\n}\n";

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
    let out = fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).unwrap();
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
    assert!(fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).is_err());
}

#[test]
fn refuses_a_missing_asset_line() {
    let text: String = manifest()
        .lines()
        .skip(2)
        .map(|l| format!("{l}\n"))
        .collect();
    let dir = release_dir(&text, true);
    assert!(fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).is_err());
}

#[test]
fn refuses_a_missing_bundle() {
    let dir = release_dir(&manifest(), false);
    let error = fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).unwrap_err();
    assert!(error.contains("is missing"), "{error}");
}

#[test]
fn refuses_a_tag_that_differs_from_dx_version() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::fill("v1.2.4", dir.path(), PINS, REPO, ok).unwrap_err();
    assert!(error.contains("differs from DX_VERSION"), "{error}");
}

#[test]
fn refuses_when_verification_fails() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, |_, _| {
        Err("bad bundle".into())
    })
    .unwrap_err();
    assert_eq!(error, "bad bundle");
}

fn filled() -> String {
    let dir = release_dir(&manifest(), true);
    fill_pins::fill("v1.2.3", dir.path(), PINS, REPO, ok).unwrap()
}

fn nothing() -> Result<(), String> {
    Ok(())
}

#[test]
fn check_passes_when_the_pins_equal_the_manifest() {
    let dir = release_dir(&manifest(), true);
    fill_pins::check("v1.2.3", dir.path(), &filled(), REPO, false, nothing, ok).unwrap();
}

#[test]
fn check_passes_empty_pins_without_fetching_or_verifying() {
    let dir = release_dir("", false);
    let fetch = || Err("fetched".to_owned());
    let verify = |_: &Path, _: &Path| Err("verified".to_owned());
    fill_pins::check("v1.2.3", dir.path(), PINS, REPO, false, fetch, verify).unwrap();
}

#[test]
fn check_fails_on_a_differing_pin() {
    let dir = release_dir(&manifest(), true);
    let asset = dx_adopt::RELEASE_ASSETS[0];
    let pins = filled().replace(&hash(1), &hash(9));
    let error =
        fill_pins::check("v1.2.3", dir.path(), &pins, REPO, false, nothing, ok).unwrap_err();
    assert!(
        error.contains("differs from the attested"),
        "{asset} {error}"
    );
}

#[test]
fn check_fails_on_mixed_pins() {
    let dir = release_dir(&manifest(), true);
    let pins = PINS.replacen(
        "\"dx-linux-arm64\": \"\"",
        &format!("\"dx-linux-arm64\": \"{}\"", hash(1)),
        1,
    );
    let error =
        fill_pins::check("v1.2.3", dir.path(), &pins, REPO, false, nothing, ok).unwrap_err();
    assert!(error.contains("mixes"), "{error}");
}

#[test]
fn check_fails_when_verification_fails() {
    let dir = release_dir(&manifest(), true);
    let verify = |_: &Path, _: &Path| Err("bad bundle".to_owned());
    let error = fill_pins::check(
        "v1.2.3",
        dir.path(),
        &filled(),
        REPO,
        false,
        nothing,
        verify,
    )
    .unwrap_err();
    assert_eq!(error, "bad bundle");
}

#[test]
fn check_fails_when_the_tag_differs_from_dx_version() {
    let dir = release_dir(&manifest(), true);
    let error =
        fill_pins::check("v1.2.4", dir.path(), &filled(), REPO, false, nothing, ok).unwrap_err();
    assert!(error.contains("differs from DX_VERSION"), "{error}");
}

#[test]
fn manifest_mode_fills_without_a_bundle() {
    let out = fill_pins::fill_from_manifest("v1.2.3", &manifest(), PINS, REPO).unwrap();
    assert_eq!(out, filled());
}

#[test]
fn manifest_mode_points_the_url_at_the_repo() {
    let out = fill_pins::fill_from_manifest("v1.2.3", &manifest(), PINS, "fork/rules_dx").unwrap();
    let want = PINS.replace("devhusdk/rules_dx", "fork/rules_dx");
    assert_eq!(out.lines().count(), want.lines().count());
    assert!(out.contains("https://github.com/fork/rules_dx/releases/download/"));
    assert!(!out.contains("devhusdk"));
}

#[test]
fn manifest_mode_refuses_a_bad_repo() {
    for repo in ["", "rules_dx", "a/b/c", "/b", "a/", "a b/c", "a/b\n"] {
        let error = fill_pins::fill_from_manifest("v1.2.3", &manifest(), PINS, repo).unwrap_err();
        assert!(error.contains("OWNER/NAME"), "{repo:?} {error}");
    }
}

#[test]
fn manifest_mode_refuses_a_tag_that_differs_from_dx_version() {
    let error = fill_pins::fill_from_manifest("v1.2.4", &manifest(), PINS, REPO).unwrap_err();
    assert!(error.contains("differs from DX_VERSION"), "{error}");
}

#[test]
fn check_filled_fails_on_empty_pins() {
    let dir = release_dir("", false);
    let error = fill_pins::check("v1.2.3", dir.path(), PINS, REPO, true, nothing, ok).unwrap_err();
    assert!(error.contains("empty"), "{error}");
}

#[test]
fn check_filled_passes_matching_pins() {
    let dir = release_dir(&manifest(), true);
    fill_pins::check("v1.2.3", dir.path(), &filled(), REPO, true, nothing, ok).unwrap();
}

#[test]
fn check_uses_the_repo_in_the_expected_pins() {
    let dir = release_dir(&manifest(), true);
    let error = fill_pins::check(
        "v1.2.3",
        dir.path(),
        &filled(),
        "fork/rules_dx",
        false,
        nothing,
        ok,
    )
    .unwrap_err();
    assert!(error.contains("differs from the attested"), "{error}");
}
