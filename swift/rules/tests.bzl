"""Analysis tests for the Swift wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftInfo")

def _library_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, SwiftInfo in target)
    asserts.true(env, CcInfo in target)
    asserts.true(env, DefaultInfo in target)
    return analysistest.end(env)

def _binary_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, DefaultInfo in target)
    asserts.true(env, target[DefaultInfo].default_runfiles != None)
    asserts.true(env, target[DefaultInfo].files_to_run.executable != None)
    return analysistest.end(env)

swift_library_provider_test = analysistest.make(_library_impl)
swift_binary_provider_test = analysistest.make(_binary_impl)

def _coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment
    script = run_env.get("CC_CODE_COVERAGE_SCRIPT", "")
    asserts.true(env, script.removesuffix(".exe").endswith("/tools/coverage/collect/collect_native"), "CC_CODE_COVERAGE_SCRIPT=" + script)
    for key, tool in [("LLVM_COV", "llvm-cov"), ("LLVM_PROFDATA", "llvm-profdata")]:
        value = run_env.get(key, "")
        asserts.true(env, value.removesuffix(".exe").endswith(tool), key + "=" + value)
        if ctx.attr.swift_tools:
            asserts.true(env, "dx_swift" in value, key + " is not the Swift toolchain tool: " + value)
    asserts.equals(env, "test_binary", run_env.get("DX_COVERAGE_CLANG_OBJECTS"))
    return analysistest.end(env)

_coverage_test = analysistest.make(
    _coverage_impl,
    attrs = {"swift_tools": attr.bool()},
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:experimental_generate_llvm_lcov": True,
    },
)

def swift_test_coverage_test(name, target_under_test, **kwargs):
    """Asserts a swift_test hands the Swift toolchain llvm-cov and llvm-profdata to the coverage collector."""
    _coverage_test(
        name = name,
        size = "small",
        target_under_test = target_under_test,
        swift_tools = select({
            "@platforms//os:linux": True,
            "//conditions:default": False,
        }),
        **kwargs
    )
