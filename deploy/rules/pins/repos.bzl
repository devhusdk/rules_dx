"""Module extension for the pinned cargo-semver-checks binaries."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", "http_file")
load(":japicmp_pins.bzl", "JAPICMP_SHA256", "JAPICMP_VERSION")
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

def _japicmp_impl(mctx):
    """Declares the pinned japicmp jar with its dependencies."""
    http_file(
        name = "japicmp_jar",
        downloaded_file_path = "japicmp.jar",
        sha256 = JAPICMP_SHA256,
        urls = ["https://repo1.maven.org/maven2/com/github/siom79/japicmp/japicmp/%s/japicmp-%s-jar-with-dependencies.jar" % (JAPICMP_VERSION, JAPICMP_VERSION)],
    )
    return mctx.extension_metadata(reproducible = True)

japicmp = module_extension(
    implementation = _japicmp_impl,
    doc = "Creates the pinned japicmp jar.",
)
