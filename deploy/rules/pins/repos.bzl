"""Module extension for the pinned cargo-semver-checks binaries."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load(":semver_checks_pins.bzl", "SEMVER_CHECKS_PLATFORMS", "SEMVER_CHECKS_VERSION")

def _semver_checks_impl(mctx):
    """Declares one pinned cargo-semver-checks archive per supported platform."""
    for key, pin in SEMVER_CHECKS_PLATFORMS.items():
        http_archive(
            name = "cargo_semver_checks_" + key,
            build_file_content = 'exports_files(["%s"])\n' % pin["exe"],
            sha256 = pin["sha256"],
            urls = ["https://github.com/obi1kenobi/cargo-semver-checks/releases/download/" + SEMVER_CHECKS_VERSION + "/cargo-semver-checks-" + pin["target"] + ".tar.gz"],
        )
    return mctx.extension_metadata(reproducible = True)

semver_checks = module_extension(
    implementation = _semver_checks_impl,
    doc = "Creates the pinned cargo-semver-checks binary for each supported platform.",
)
