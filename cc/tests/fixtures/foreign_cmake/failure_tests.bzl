"""Analysis tests for foreign cmake failure modes."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:failure_test.bzl", "failure_test_rule")

unknown_generator_test = failure_test_rule()

_CMAKE_TOOLCHAIN = "@rules_foreign_cc//toolchains:cmake_toolchain"

def _cmake_toolchain_probe_impl(ctx):
    if not ctx.toolchains[_CMAKE_TOOLCHAIN]:
        fail("No toolchain found for " + _CMAKE_TOOLCHAIN)
    return [DefaultInfo()]

cmake_toolchain_probe = rule(
    implementation = _cmake_toolchain_probe_impl,
    toolchains = [config_common.toolchain_type(_CMAKE_TOOLCHAIN, mandatory = False)],
)

missing_cmake_toolchain_test = failure_test_rule({
    "//command_line_option:extra_execution_platforms": [str(Label(":linux_riscv64_exec"))],
})

def _cmake_toolchain_present_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, DefaultInfo in ctx.attr.target_under_test)
    return analysistest.end(env)

cmake_toolchain_present_test = analysistest.make(_cmake_toolchain_present_impl)
