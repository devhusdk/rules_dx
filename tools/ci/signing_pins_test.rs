use dx_testing::read_runfiles;

const SIGNING_BZL: &str = "deploy/private/release/signing.bzl";
const COSIGN_PINS: &str = "deploy/private/release/cosign_pins.bzl";
const MODULE: &str = "MODULE.bazel";
const RELEASE_LIB: &str = "deploy/private/release/src/lib.rs";

#[test]
fn the_cosign_pin_lives_only_in_cosign_pins_bzl() {
    let pins = read_runfiles(COSIGN_PINS);
    let version = dx_testing::starlark_const(&pins, "COSIGN_VERSION", COSIGN_PINS);
    assert!(
        version.starts_with('v'),
        "{COSIGN_PINS} COSIGN_VERSION is {version}"
    );
    let mut pinned = vec![version];
    for name in [
        "COSIGN_SHA256_LINUX_AMD64",
        "COSIGN_SHA256_LINUX_ARM64",
        "COSIGN_SHA256_DARWIN_ARM64",
        "COSIGN_SHA256_WINDOWS_AMD64",
    ] {
        let sha = dx_testing::starlark_const(&pins, name, COSIGN_PINS);
        assert!(
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{COSIGN_PINS} {name} is {sha}, want 64 lowercase hex characters"
        );
        pinned.push(sha);
    }
    for (rel, text) in [
        (SIGNING_BZL, read_runfiles(SIGNING_BZL)),
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
