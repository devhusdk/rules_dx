"""Unit tests for signing selection (live successor to closed /)."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//quality/artifacts:hub.bzl", "TOOL_PLATFORMS")
load(":cosign_pins.bzl", "COSIGN_PLATFORMS")
load(":signing.bzl", "SIGNING_BUNDLE_MEDIA_TYPE", "SIGNING_COSIGN_VERSION", "SIGNING_ISSUER", "SIGNING_TRUST_ROOT", "signing_bundle_media_error", "signing_bundle_names", "signing_cosign_error", "signing_identity_error")

def signing_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "signing trust root is Sigstore TUF",
                SIGNING_TRUST_ROOT,
                "https://tuf-repo-cdn.sigstore.dev",
            ),
            expect_equal(
                "signing issuer is GitHub OIDC",
                SIGNING_ISSUER,
                "https://token.actions.githubusercontent.com",
            ),
            expect_equal(
                "signing cosign version is pinned v2.6.5",
                SIGNING_COSIGN_VERSION,
                "v2.6.5",
            ),
            expect_equal(
                "signing cosign pins cover exactly the tool platforms",
                sorted(COSIGN_PLATFORMS.keys()),
                sorted(TOOL_PLATFORMS),
            ),
            expect_equal(
                "signing cosign sha is pinned for linux-amd64",
                COSIGN_PLATFORMS["linux_x86_64"]["sha256"],
                "c3b4f5410e608af03a5eb0aaac84a4313d8da131248e08ff1759ac70c79d1644",
            ),
            expect_equal(
                "signing cosign shas are 64 characters",
                [key for key, pin in COSIGN_PLATFORMS.items() if len(pin["sha256"]) != 64],
                [],
            ),
            expect_equal(
                "signing cosign shas are pairwise distinct",
                len({pin["sha256"]: True for pin in COSIGN_PLATFORMS.values()}),
                len(COSIGN_PLATFORMS),
            ),
            expect_equal(
                "signing cosign windows binary is cosign.exe",
                COSIGN_PLATFORMS["windows_x86_64"]["exe"],
                "cosign.exe",
            ),
            expect_equal(
                "signing bundle media type is Sigstore bundle v0.3",
                SIGNING_BUNDLE_MEDIA_TYPE,
                "application/vnd.dev.sigstore.bundle.v0.3+json",
            ),
            expect_equal(
                "signing cosign pin accepts v2.6.5",
                signing_cosign_error("v2.6.5"),
                "",
            ),
            expect_equal(
                "signing cosign pin rejects a floating version",
                signing_cosign_error("v2.6.50"),
                "signing: invalid cosign version 'v2.6.50': want 'v2.6.5' (pinned)",
            ),
            expect_equal(
                "signing bundle media accepts v0.3",
                signing_bundle_media_error("application/vnd.dev.sigstore.bundle.v0.3+json"),
                "",
            ),
            expect_equal(
                "signing bundle media rejects an undeclared version",
                signing_bundle_media_error("application/vnd.dev.sigstore.bundle.v0.2+json"),
                "signing: invalid bundle media type 'application/vnd.dev.sigstore.bundle.v0.2+json': want 'application/vnd.dev.sigstore.bundle.v0.3+json' (Sigstore bundle v0.3; 0.1/0.2 only if declared)",
            ),
            expect_equal(
                "signing bundle names derive per instance",
                signing_bundle_names("release"),
                ("release.bundle", "release.attestation"),
            ),
            expect_equal(
                "signing identity accepts the release workflow identity",
                signing_identity_error("https://github.com/devhusdk/rules_dx/.github/workflows/release.yml@refs/tags/v1.2.3", "https://token.actions.githubusercontent.com"),
                "",
            ),
            expect_equal(
                "signing identity rejects an empty identity",
                signing_identity_error("", "https://token.actions.githubusercontent.com"),
                "signing: invalid identity '': want the owner-approved release workflow identity",
            ),
            expect_equal(
                "signing identity rejects a foreign issuer",
                signing_identity_error("https://github.com/devhusdk/rules_dx/.github/workflows/release.yml@refs/tags/v1.2.3", "https://evil.example"),
                "signing: invalid issuer 'https://evil.example': want 'https://token.actions.githubusercontent.com' (Sigstore keyless via GitHub OIDC)",
            ),
        ],
    )
