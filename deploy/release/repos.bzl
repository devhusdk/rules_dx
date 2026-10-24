"""Module extension for the pinned cosign binary."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load(":cosign_pins.bzl", "COSIGN_SHA256_LINUX_AMD64", "COSIGN_VERSION")

def _cosign_impl(mctx):
    """Declares the pinned cosign linux-amd64 binary."""
    http_file(
        name = "cosign_linux_amd64",
        executable = True,
        sha256 = COSIGN_SHA256_LINUX_AMD64,
        urls = ["https://github.com/sigstore/cosign/releases/download/" + COSIGN_VERSION + "/cosign-linux-amd64"],
    )
    return mctx.extension_metadata(reproducible = True)

cosign = module_extension(
    implementation = _cosign_impl,
    doc = "Creates the pinned cosign linux-amd64 binary.",
)
