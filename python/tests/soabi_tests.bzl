"""Analysis tests for the SOABI of the rules_dx python_3_12 py_cc toolchains."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_PY_CC_TOOLCHAIN_TYPE = "@rules_python//python/cc:toolchain_type"

def _soabi_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[platform_common.ToolchainInfo].py_cc_toolchain
    asserts.equals(env, ctx.attr.soabi, info.soabi)
    asserts.equals(env, "3.12", info.python_version)
    asserts.true(env, info.headers != None, "py_cc_toolchain has no headers")
    return analysistest.end(env)

soabi_test = analysistest.make(
    _soabi_impl,
    attrs = {"soabi": attr.string(mandatory = True)},
)

def _resolved_py_cc_toolchain_impl(ctx):
    return [ctx.toolchains[_PY_CC_TOOLCHAIN_TYPE]]

resolved_py_cc_toolchain = rule(
    implementation = _resolved_py_cc_toolchain_impl,
    doc = "Returns the ToolchainInfo that toolchain resolution picks for the py_cc toolchain type.",
    toolchains = [_PY_CC_TOOLCHAIN_TYPE],
)

soabi_resolution_test = analysistest.make(
    _soabi_impl,
    attrs = {"soabi": attr.string(mandatory = True)},
    config_settings = {
        "//command_line_option:platforms": str(Label("//tools/platforms:macos_arm64")),
        str(Label("//config:toolchains")): True,
    },
)
