"""Module extension for tools/bazelrc."""

load(":repos.bzl", "bazel_version_repo")


def _bazel_version_impl(mctx):
    # Reading the file through the extension keeps the root package out of every Rust target.
    version = mctx.read(Label("//:.bazelversion")).strip()
    bazel_version_repo(name = "dx_bazel_version", version = version)
    return mctx.extension_metadata(reproducible = True)


bazel_version = module_extension(
    implementation = _bazel_version_impl,
    doc = "Creates @dx_bazel_version with the version from .bazelversion as a rustc env file.",
)
