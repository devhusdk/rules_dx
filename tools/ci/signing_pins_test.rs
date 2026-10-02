use dx_testing::read_runfiles;

const SIGNING_BZL: &str = "deploy/release/signing.bzl";
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
fn the_cosign_version_and_bundle_media_type_match_the_starlark_pins() {
    let signing = read_runfiles(SIGNING_BZL);
    let release = read_runfiles(RELEASE_LIB);
    for name in ["SIGNING_COSIGN_VERSION", "SIGNING_BUNDLE_MEDIA_TYPE"] {
        assert_eq!(
            rust_const(&release, RELEASE_LIB, name),
            starlark_const(&signing, name),
            "{RELEASE_LIB} names {SIGNING_BZL} {name} as the pin it enforces"
        );
    }
}

#[test]
fn the_ghcr_cosign_fetch_matches_the_starlark_pins() {
    let signing = read_runfiles(SIGNING_BZL);
    let ghcr = read_runfiles(GHCR_WORKFLOW);
    for name in ["COSIGN_VERSION", "COSIGN_SHA256_LINUX_AMD64"] {
        let value = starlark_const(&signing, format!("SIGNING_{name}").as_str());
        let declaration = format!("{name}=\"{value}\"");
        assert_eq!(
            ghcr.matches(&declaration).count(),
            1,
            "{GHCR_WORKFLOW} must fetch the {SIGNING_BZL} {name} pin {value}"
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
