"""Module extension for the pinned nested Bazel binaries."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load(":bazel_pins.bzl", "NESTED_BAZEL_SHA256", "NESTED_BAZEL_VERSION")

def _nested_bazel_impl(mctx):
    """Declares one pinned Bazel binary per platform."""
    for platform, sha256 in NESTED_BAZEL_SHA256.items():
        http_file(
            name = "nested_bazel_" + platform.replace("-", "_"),
            downloaded_file_path = "bazel",
            executable = True,
            sha256 = sha256,
            urls = ["https://github.com/bazelbuild/bazel/releases/download/{v}/bazel-{v}-{p}".format(
                v = NESTED_BAZEL_VERSION,
                p = platform,
            )],
        )
    return mctx.extension_metadata(reproducible = True)

nested_bazel = module_extension(
    implementation = _nested_bazel_impl,
    doc = "Creates the pinned Bazel binaries for nested Bazel tests.",
)
