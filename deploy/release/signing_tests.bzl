"""Unit tests for signing selection (live successor to closed /)."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":signing.bzl", "SIGNING_BUNDLE_MEDIA_TYPE", "SIGNING_COSIGN_SHA256_LINUX_AMD64", "SIGNING_COSIGN_VERSION", "SIGNING_ISSUER", "SIGNING_TRUST_ROOT", "signing_bundle_media_error", "signing_bundle_names", "signing_cosign_error", "signing_cosign_sha_error", "signing_identity_error")

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
                "signing cosign version is pinned v2.4.1",
                SIGNING_COSIGN_VERSION,
                "v2.4.1",
            ),
            expect_equal(
                "signing cosign sha is pinned linux-amd64",
                SIGNING_COSIGN_SHA256_LINUX_AMD64,
                "8b24b946dd5809c6bd93de08033bcf6bc0ed7d336b7785787c080f574b89249b",
            ),
            expect_equal(
                "signing bundle media type is Sigstore bundle v0.3",
                SIGNING_BUNDLE_MEDIA_TYPE,
                "application/vnd.dev.sigstore.bundle.v0.3+json",
            ),
            expect_equal(
                "signing cosign pin accepts v2.4.1",
                signing_cosign_error("v2.4.1"),
                "",
            ),
            expect_equal(
                "signing cosign pin rejects a floating version",
                signing_cosign_error("v2.4.0"),
                "signing: invalid cosign version 'v2.4.0': want 'v2.4.1' (pinned)",
            ),
            expect_equal(
                "signing cosign sha accepts the pinned linux-amd64 sha",
                signing_cosign_sha_error("8b24b946dd5809c6bd93de08033bcf6bc0ed7d336b7785787c080f574b89249b"),
                "",
            ),
            expect_equal(
                "signing cosign sha rejects a floating sha",
                signing_cosign_sha_error("0" * 64),
                "signing: invalid cosign sha '0000000000000000000000000000000000000000000000000000000000000000': want '8b24b946dd5809c6bd93de08033bcf6bc0ed7d336b7785787c080f574b89249b' (pinned)",
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
