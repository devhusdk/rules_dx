use dx_testing::read_runfiles;

const SIGNING_BZL: &str = "deploy/release/signing.bzl";
const COSIGN_PINS: &str = "deploy/release/cosign_pins.bzl";
const MODULE: &str = "MODULE.bazel";
const RELEASE_LIB: &str = "deploy/release/src/lib.rs";
const GHCR_WORKFLOW: &str = ".github/workflows/ghcr.yml";

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
