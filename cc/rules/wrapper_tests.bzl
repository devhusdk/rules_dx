"""Wrapper conformance tests for C/C++."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "cc_coverage_features")

def cc_wrapper_contract_tests(name):
    """Instantiates C/C++ wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["c", "cpp"],
        extra_checks = [
            expect_equal("coverage off keeps features", cc_coverage_features(["a"], False), ["a"]),
            expect_equal("coverage on adds llvm maps", cc_coverage_features(["a"], True), ["a", "llvm_coverage_map_format", "-gcc_coverage_map_format"]),
            expect_equal("coverage on is idempotent", cc_coverage_features(["llvm_coverage_map_format"], True), ["llvm_coverage_map_format", "-gcc_coverage_map_format"]),
        ],
    )

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    run_env = analysistest.target_under_test(env)[RunEnvironmentInfo].environment
    if ctx.attr.expect_collector:
        asserts.true(env, run_env.get("CC_CODE_COVERAGE_SCRIPT", "").endswith("/tools/coverage/collect/collect_native"), "CC_CODE_COVERAGE_SCRIPT=" + run_env.get("CC_CODE_COVERAGE_SCRIPT", ""))
        asserts.true(env, run_env.get("LLVM_COV", "").endswith("llvm-cov"), "LLVM_COV=" + run_env.get("LLVM_COV", ""))
        asserts.true(env, run_env.get("LLVM_PROFDATA", "").endswith("llvm-profdata"), "LLVM_PROFDATA=" + run_env.get("LLVM_PROFDATA", ""))
    else:
        for key in ["CC_CODE_COVERAGE_SCRIPT", "LLVM_COV", "LLVM_PROFDATA"]:
            asserts.false(env, key in run_env, key + " set without coverage")
    return analysistest.end(env)

_coverage_env_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"expect_collector": attr.bool(default = True)},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_env_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"expect_collector": attr.bool(default = False)},
)

def cc_coverage_env_tests(name, target):
    """Asserts a cc_test gets the dx native collector only under coverage."""
    _coverage_env_on_test(name = name + "_on", size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", tags = ["no-coverage"], target_under_test = target)
    native.test_suite(name = name, tests = [name + "_on", name + "_off"])
