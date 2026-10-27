use dx_testing::read_runfiles;

const SIGNING_BZL: &str = "deploy/release/signing.bzl";
const COSIGN_PINS: &str = "deploy/release/cosign_pins.bzl";
const MODULE: &str = "MODULE.bazel";
const RELEASE_LIB: &str = "deploy/release/src/lib.rs";
const INSTALL_LIB: &str = "deploy/install/src/lib.rs";
const GHCR_WORKFLOW: &str = ".github/workflows/ghcr.yml";

fn starlark_const(text: &str, name: &str) -> String {
    dx_testing::starlark_const(text, name, SIGNING_BZL)
}

fn rust_const(text: &str, rel: &str, name: &str) -> String {
    let prefix = format!("pub const {name}: &str = \"");
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("{rel} has no {name}"))
        .trim_end_matches(';')
        .trim_end_matches('"')
        .to_owned()
}

#[test]
fn the_trust_root_is_one_pin_in_every_declaration() {
    let signing = read_runfiles(SIGNING_BZL);
    let pinned = starlark_const(&signing, "SIGNING_TRUST_ROOT");
    assert_eq!(
        rust_const(
            &read_runfiles(RELEASE_LIB),
            RELEASE_LIB,
            "SIGNING_TRUST_ROOT"
        ),
        pinned,
        "{RELEASE_LIB} prints the trust root {SIGNING_BZL} validates against"
    );
    assert_eq!(
        rust_const(&read_runfiles(INSTALL_LIB), INSTALL_LIB, "TRUST_ROOT"),
        pinned,
        "{INSTALL_LIB} reports the trust root the release signs on"
    );
}

#[test]
fn the_issuer_is_one_pin_in_every_declaration() {
    let signing = read_runfiles(SIGNING_BZL);
    let pinned = starlark_const(&signing, "SIGNING_ISSUER");
    assert_eq!(
        rust_const(
            &read_runfiles(RELEASE_LIB),
            RELEASE_LIB,
            "SIGNING_ISSUER_DEFAULT"
        ),
        pinned,
        "{RELEASE_LIB} signs with the issuer {SIGNING_BZL} validates against"
    );
    assert_eq!(
        rust_const(&read_runfiles(INSTALL_LIB), INSTALL_LIB, "DEFAULT_ISSUER"),
        pinned,
        "{INSTALL_LIB} names the issuer the release is verified against"
    );
}

#[test]
fn the_bundle_media_type_matches_the_starlark_pin() {
    let name = "SIGNING_BUNDLE_MEDIA_TYPE";
    assert_eq!(
        rust_const(&read_runfiles(RELEASE_LIB), RELEASE_LIB, name),
        starlark_const(&read_runfiles(SIGNING_BZL), name),
        "{RELEASE_LIB} names {SIGNING_BZL} {name} as the pin it enforces"
    );
}

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
        (GHCR_WORKFLOW, read_runfiles(GHCR_WORKFLOW)),
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
        "{MODULE} fetches cosign outside deploy/release/repos.bzl"
    );
}

#[test]
fn the_ghcr_cosign_fetch_reads_cosign_pins_bzl() {
    let ghcr = read_runfiles(GHCR_WORKFLOW);
    for name in ["COSIGN_VERSION", "COSIGN_SHA256_LINUX_AMD64"] {
        let reader =
            format!("{name}=\"$(sed -n 's/^{name} = \"\\(.*\\)\"$/\\1/p' {COSIGN_PINS})\"");
        assert_eq!(
            ghcr.matches(&reader).count(),
            1,
            "{GHCR_WORKFLOW} must read {name} from {COSIGN_PINS}"
        );
    }
}

#[test]
fn no_declaration_repeats_a_pinned_url_outside_its_own_constant() {
    let signing = read_runfiles(SIGNING_BZL);
    let trust_root = starlark_const(&signing, "SIGNING_TRUST_ROOT");
    let issuer = starlark_const(&signing, "SIGNING_ISSUER");
    for (rel, text) in [
        (SIGNING_BZL, &signing),
        (RELEASE_LIB, &read_runfiles(RELEASE_LIB)),
        (INSTALL_LIB, &read_runfiles(INSTALL_LIB)),
    ] {
        for url in [&trust_root, &issuer] {
            assert_eq!(
                text.matches(url.as_str()).count(),
                1,
                "{rel} repeats {url} outside its constant, so one copy can drift"
            );
        }
    }
}

#[test]
fn the_ghcr_push_attests_the_pushed_digest() {
    let ghcr = read_runfiles(GHCR_WORKFLOW);
    assert!(
        !ghcr.contains("gh attestation create"),
        "{GHCR_WORKFLOW} calls gh attestation create, which gh does not have"
    );
    assert_eq!(
        ghcr.matches("uses: actions/attest@").count(),
        1,
        "{GHCR_WORKFLOW} must attest the pushed image with actions/attest once"
    );
    for needle in [
        "push-to-registry: true",
        "subject-digest: ${{ steps.push.outputs.digest }}",
    ] {
        assert!(
            ghcr.contains(needle),
            "{GHCR_WORKFLOW} must set {needle} on the attestation step"
        );
    }
    for line in ghcr.lines().filter(|line| line.contains("attestation")) {
        assert!(
            !line.contains("|| echo") && !line.contains("|| true"),
            "{GHCR_WORKFLOW} hides an attestation failure: {line}"
        );
    }
}
