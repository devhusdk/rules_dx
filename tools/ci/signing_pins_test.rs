use dx_testing::read_runfiles;

const SIGNING_BZL: &str = "deploy/private/release/signing.bzl";
const PINS_JSON: &str = "deploy/private/release/pins.json";
const COSIGN_PINS: &str = "deploy/private/release/cosign_pins.bzl";
const MODULE: &str = "MODULE.bazel";
const RELEASE_LIB: &str = "deploy/private/release/src/lib.rs";

#[test]
fn the_cosign_pin_lives_only_in_cosign_pins_bzl() {
    let pins: serde_json::Value = serde_json::from_str(&read_runfiles(PINS_JSON))
        .unwrap_or_else(|e| panic!("{PINS_JSON}: {e}"));
    let version = pins["version"].as_str().unwrap_or_default().to_string();
    assert!(
        version.starts_with('v'),
        "{PINS_JSON} version is {version}"
    );
    let mut pinned = vec![version];
    let cells = pins["cells"].as_object().expect("cells object");
    assert!(!cells.is_empty(), "{PINS_JSON} has no cells");
    for (name, cell) in cells {
        let sha = cell["sha256"].as_str().unwrap_or_default().to_string();
        assert!(
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{PINS_JSON} {name} is {sha}, want 64 lowercase hex characters"
        );
        pinned.push(sha);
    }
    for (rel, text) in [
        (SIGNING_BZL, read_runfiles(SIGNING_BZL)),
        (COSIGN_PINS, read_runfiles(COSIGN_PINS)),
        (RELEASE_LIB, read_runfiles(RELEASE_LIB)),
        (MODULE, read_runfiles(MODULE)),
    ] {
        for value in &pinned {
            assert!(
                !text.contains(&format!("\"{value}\"")),
                "{rel} repeats the {COSIGN_PINS} pin {value}"
            );
        }
    }
    assert!(
        !read_runfiles(MODULE).contains("sigstore/cosign/releases"),
        "{MODULE} fetches cosign outside deploy/private/release/repos.bzl"
    );
}
