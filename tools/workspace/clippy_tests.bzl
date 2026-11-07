"""Checks that the clippy config flag points at this package, not the root one."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _clippy_config_impl(ctx):
    env = analysistest.begin(ctx)
    files = analysistest.target_under_test(env)[DefaultInfo].files.to_list()
    asserts.equals(env, 1, len(files), "clippy config files: " + str(files))
    if files:
        asserts.equals(env, Label("//tools/workspace:clippy.toml"), files[0].owner)
    return analysistest.end(env)

_clippy_config_test = analysistest.make(_clippy_config_impl)

def clippy_config_test(name):
    """Fails when .bazelrc points the clippy config flag outside //tools/workspace."""
    native.filegroup(
        name = name + "_flag",
        testonly = True,
        srcs = ["@rules_rust//rust/settings:clippy.toml"],
    )
    _clippy_config_test(
        name = name,
        size = "small",
        target_under_test = name + "_flag",
    )
