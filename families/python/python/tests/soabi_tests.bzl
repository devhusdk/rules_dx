"""Analysis tests for the SOABI of the rules_py interpreter toolchains."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _soabi_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[platform_common.ToolchainInfo].py_cc_toolchain
    asserts.equals(env, ctx.attr.soabi, info.soabi)
    return analysistest.end(env)

soabi_test = analysistest.make(
    _soabi_impl,
    attrs = {"soabi": attr.string(mandatory = True)},
)
