"""Module extension for the pinned cosign binaries."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load(":cosign_pins.bzl", "COSIGN_PLATFORMS", "COSIGN_VERSION")

def _cosign_impl(mctx):
    """Declares one pinned cosign binary per supported platform."""
    for key, pin in COSIGN_PLATFORMS.items():
        http_file(
            name = "cosign_" + key,
            downloaded_file_path = pin["exe"],
            executable = True,
            sha256 = pin["sha256"],
            urls = ["https://github.com/sigstore/cosign/releases/download/" + COSIGN_VERSION + "/" + pin["asset"]],
        )
    return mctx.extension_metadata(reproducible = True)

cosign = module_extension(
    implementation = _cosign_impl,
    doc = "Creates the pinned cosign binary for each supported platform.",
)
