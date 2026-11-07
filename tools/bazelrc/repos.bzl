"""Module extension that hands the pinned Bazel version to the dx_preset crate."""

def _bazel_version_repo_impl(rctx):
    rctx.file("rustc_env", "DX_PRESET_BAZEL_VERSION={}\n".format(rctx.attr.version))
    rctx.file("BUILD.bazel", 'exports_files(["rustc_env"], visibility = ["//visibility:public"])\n')

_bazel_version_repo = repository_rule(
    implementation = _bazel_version_repo_impl,
    attrs = {"version": attr.string(mandatory = True)},
)

def _bazel_version_impl(mctx):
    # Reading the file through the extension keeps the root package out of every Rust target.
    version = mctx.read(Label("//:.bazelversion")).strip()
    _bazel_version_repo(name = "dx_bazel_version", version = version)
    return mctx.extension_metadata(reproducible = True)

bazel_version = module_extension(
    implementation = _bazel_version_impl,
    doc = "Creates @dx_bazel_version with the version from .bazelversion as a rustc env file.",
)
