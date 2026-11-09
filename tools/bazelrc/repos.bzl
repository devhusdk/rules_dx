"""Module extension that hands the pinned Bazel version to the dx_preset crate."""

def _bazel_version_repo_impl(rctx):
    rctx.file("rustc_env", "DX_PRESET_BAZEL_VERSION={}\n".format(rctx.attr.version))
    rctx.file("BUILD.bazel", 'exports_files(["rustc_env"], visibility = ["//visibility:public"])\n')

bazel_version_repo = repository_rule(
    implementation = _bazel_version_repo_impl,
    attrs = {"version": attr.string(mandatory = True)},
)
