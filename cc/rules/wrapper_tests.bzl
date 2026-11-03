"""Wrapper conformance tests for C, C++ and Objective-C."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_cc//cc/common:objc_info.bzl", "ObjcInfo")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":defs.bzl", "cc_coverage_features", "cc_coverage_unsupported")

def cc_wrapper_contract_tests(name):
    """Instantiates C, C++ and Objective-C wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["c", "cpp", "objc", "objcpp"],
        wrappers = {
            "binary": "//cc/tests/fixtures/hello:hello",
            "library": "//cc/tests/fixtures/hello:hello_lib",
            "test": "//cc/tests/fixtures/hello:hello_test",
        },
        extra_checks = [
            expect_equal("coverage off keeps features", cc_coverage_features(["a"], False), ["a"]),
            expect_equal("coverage on adds llvm maps", cc_coverage_features(["a"], True), ["a", "llvm_coverage_map_format", "-gcc_coverage_map_format"]),
            expect_equal("coverage on is idempotent", cc_coverage_features(["llvm_coverage_map_format"], True), ["llvm_coverage_map_format", "-gcc_coverage_map_format"]),
            expect_equal("clang keeps llvm maps", cc_coverage_features([], True, "clang"), ["llvm_coverage_map_format", "-gcc_coverage_map_format"]),
            expect_equal("gcc gets no llvm maps", cc_coverage_features(["a"], True, "gcc"), ["a"]),
            expect_equal("msvc gets no llvm maps", cc_coverage_features(["a"], True, "msvc-cl"), ["a"]),
            expect_equal("custom compiler gets no llvm maps", cc_coverage_features(["a"], True, "icx"), ["a"]),
            expect_equal("llvm coverage supported", cc_coverage_unsupported("clang"), ""),
            expect_equal("unknown compiler not blocked", cc_coverage_unsupported("icx"), ""),
            expect_equal("gcc coverage reported", "gcc" in cc_coverage_unsupported("gcc"), True),
            expect_equal("msvc coverage reported", "msvc-cl" in cc_coverage_unsupported("msvc-cl"), True),
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

def _objc_library_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, ObjcInfo in target, "objc_library lost ObjcInfo")
    asserts.true(env, CcInfo in target, "objc_library lost CcInfo")
    asserts.false(env, QualitySourcesInfo in target, "objc_library must be the upstream rule, not a forwarder")
    return analysistest.end(env)

_objc_library_test = analysistest.make(_objc_library_impl)

def objc_library_tests(name, target, **kwargs):
    """Asserts objc_library is the plain upstream rule: ObjcInfo and CcInfo, no forwarder provider."""
    _objc_library_test(name = name, size = "small", target_under_test = target, **kwargs)

def _build_id_impl(ctx):
    env = analysistest.begin(ctx)
    links = [a for a in analysistest.target_actions(env) if a.mnemonic == "CppLink"]
    asserts.equals(env, 1, len(links))
    asserts.true(env, "-Wl,--build-id=sha1" in links[0].argv, "link argv: " + " ".join(links[0].argv))
    return analysistest.end(env)

_build_id_test = analysistest.make(_build_id_impl)

def cc_build_id_test(name, target):
    """Asserts a cc_binary link action asks for a sha1 build-id on Linux."""
    _build_id_test(
        name = name,
        size = "small",
        target_under_test = target,
        target_compatible_with = ["@platforms//os:linux"],
    )
