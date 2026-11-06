"""Wrapper conformance tests for C, C++ and Objective-C."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_cc//cc/common:objc_info.bzl", "ObjcInfo")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":defs.bzl", "cc_coverage_features", "cc_coverage_mismatch", "cc_coverage_unsupported", "cc_param_file_features")

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
            expect_equal("gcc coverage supported", cc_coverage_unsupported("gcc"), ""),
            expect_equal("gcc with llvm maps names the flag", "--compiler=gcc" in cc_coverage_mismatch("gcc", True), True),
            expect_equal("gcc without llvm maps passes", cc_coverage_mismatch("gcc", False), ""),
            expect_equal("clang with llvm maps passes", cc_coverage_mismatch("clang", True), ""),
            expect_equal("icx with llvm maps passes", cc_coverage_mismatch("icx", True), ""),
            expect_equal("space in srcs drops the params file", cc_param_file_features({}, ["dir with space/a.cc"], None), ["-compiler_param_file"]),
            expect_equal("quote in local_defines drops the params file", cc_param_file_features({"local_defines": ['NAME="x"']}, ["a.cc"], None), ["-compiler_param_file"]),
            expect_equal("plain target keeps the params file", cc_param_file_features({"copts": ["-O2"]}, ["a.cc"], ["a.h"]), []),
            expect_equal("select is not inspected", cc_param_file_features({"copts": select({"//conditions:default": ["-DA B"]})}, ["a.cc"], None), []),
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

def _gcc_asan_env_impl(ctx):
    env = analysistest.begin(ctx)
    run_env = analysistest.target_under_test(env)[RunEnvironmentInfo].environment
    if ctx.attr.expect:
        asserts.equals(env, ctx.attr.expect, run_env.get("ASAN_OPTIONS"))
    else:
        asserts.false(env, "ASAN_OPTIONS" in run_env, "ASAN_OPTIONS set without GCC")
    return analysistest.end(env)

_gcc_asan_env_gcc_test = analysistest.make(
    _gcc_asan_env_impl,
    attrs = {"expect": attr.string()},
    config_settings = {str(Label("//cc/toolchains:compiler")): "gcc"},
)

_gcc_asan_env_llvm_test = analysistest.make(
    _gcc_asan_env_impl,
    attrs = {"expect": attr.string()},
    config_settings = {str(Label("//cc/toolchains:compiler")): "llvm"},
)

def cc_gcc_asan_env_tests(name, target, expect, expect_llvm = "", **kwargs):
    """Asserts cc_test sets ASAN_OPTIONS to expect under GCC and to expect_llvm (default unset) under LLVM."""
    _gcc_asan_env_gcc_test(name = name + "_gcc", size = "small", expect = expect, target_under_test = target, **kwargs)
    _gcc_asan_env_llvm_test(name = name + "_llvm", size = "small", expect = expect_llvm, target_under_test = target, **kwargs)
    native.test_suite(name = name, tests = [name + "_gcc", name + "_llvm"], **kwargs)

def _external_toolchain_asan_test(toolchain):
    return analysistest.make(
        _gcc_asan_env_impl,
        attrs = {"expect": attr.string()},
        config_settings = {"//command_line_option:extra_toolchains": [toolchain]},
    )

_asan_env_external_gcc_test = _external_toolchain_asan_test(str(Label("//cc/toolchains/external:gcc_toolchain")))
_asan_env_external_icx_test = _external_toolchain_asan_test(str(Label("//cc/toolchains/external:icx_toolchain")))

def cc_external_asan_env_tests(name, target, **kwargs):
    """Asserts cc_test sets ASAN_OPTIONS under an external GCC toolchain and not under another external compiler."""
    _asan_env_external_gcc_test(name = name + "_gcc", size = "small", expect = "verify_asan_link_order=0", target_under_test = target, **kwargs)
    _asan_env_external_icx_test(name = name + "_icx", size = "small", target_under_test = target, **kwargs)
    native.test_suite(name = name, tests = [name + "_gcc", name + "_icx"], **kwargs)

def _coverage_mismatch_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "--compiler=gcc")
    return analysistest.end(env)

def _icx_coverage_impl(ctx):
    env = analysistest.begin(ctx)
    return analysistest.end(env)

def _coverage_flag_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "does not ship gcov")
    return analysistest.end(env)

def _external_coverage_test(toolchain, impl, **settings):
    return analysistest.make(
        impl,
        expect_failure = impl != _icx_coverage_impl,
        config_settings = {
            "//command_line_option:collect_code_coverage": True,
            "//command_line_option:extra_toolchains": [toolchain],
        } | settings,
    )

def _gcov_env_impl(ctx):
    env = analysistest.begin(ctx)
    run_env = analysistest.target_under_test(env)[RunEnvironmentInfo].environment
    script = run_env.get("CC_CODE_COVERAGE_SCRIPT", "")
    gcov = run_env.get("GCOV", "")
    asserts.true(env, script.endswith("/tools/coverage/collect/collect_native"), "CC_CODE_COVERAGE_SCRIPT=" + script)
    asserts.true(env, gcov.endswith("gcov"), "GCOV=" + gcov)
    for key in ["LLVM_COV", "LLVM_PROFDATA"]:
        asserts.false(env, key in run_env, key + " set under gcc")
    return analysistest.end(env)

_coverage_env_gcc_test = analysistest.make(
    _gcov_env_impl,
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        str(Label("//cc/toolchains:compiler")): "gcc",
    },
)

_coverage_external_gcc_test = _external_coverage_test(str(Label("//cc/toolchains/external:gcc_toolchain")), _coverage_mismatch_impl)
_coverage_external_gcc_flag_test = _external_coverage_test(str(Label("//cc/toolchains/external:gcc_toolchain")), _coverage_flag_impl, **{"//command_line_option:compiler": "gcc"})
_coverage_external_icx_test = _external_coverage_test(str(Label("//cc/toolchains/external:icx_toolchain")), _icx_coverage_impl)

def cc_external_coverage_tests(name, target, **kwargs):
    """Asserts cc_test coverage fails fast under an external GCC toolchain without --compiler, gets past that check with it (the fake toolchain has no gcov), and leaves unknown compilers alone."""
    _coverage_external_gcc_test(name = name + "_gcc", size = "small", target_under_test = target, **kwargs)
    _coverage_external_gcc_flag_test(name = name + "_gcc_flag", size = "small", target_under_test = target, **kwargs)
    _coverage_external_icx_test(name = name + "_icx", size = "small", target_under_test = target, **kwargs)
    native.test_suite(name = name, tests = [name + "_gcc", name + "_gcc_flag", name + "_icx"], **kwargs)

def cc_gcov_coverage_env_test(name, target):
    """Asserts a cc_test under the GCC toolchain gets GCOV and no LLVM coverage tools."""
    _coverage_env_gcc_test(
        name = name,
        size = "small",
        target_compatible_with = [
            "@platforms//os:linux",
            "@platforms//cpu:x86_64",
        ],
        target_under_test = target,
    )

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

def _static_link_impl(ctx):
    env = analysistest.begin(ctx)
    links = [a for a in analysistest.target_actions(env) if a.mnemonic == "CppLink"]
    asserts.equals(env, 1, len(links))
    dylibs = [f.path for f in links[0].inputs.to_list() if f.path.endswith(tuple(ctx.attr.suffixes))]
    asserts.equals(env, [], dylibs)
    return analysistest.end(env)

_static_link_test = analysistest.make(_static_link_impl, attrs = {"suffixes": attr.string_list()})

def cc_macos_static_link_test(name, target):
    """Asserts a cc_test link on macOS takes no dynamic library inputs."""
    _static_link_test(
        name = name,
        size = "small",
        target_under_test = target,
        suffixes = [".dylib"],
        target_compatible_with = ["@platforms//os:macos"],
    )

def cc_windows_static_link_test(name, target):
    """Asserts a cc_test link on Windows takes no DLL import library inputs."""
    _static_link_test(
        name = name,
        size = "small",
        target_under_test = target,
        suffixes = [".if.lib"],
        target_compatible_with = ["@platforms//os:windows"],
    )
