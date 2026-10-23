"""Analysis tests for the Dart wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartCompileInfo", "DartInfo")
load("//dart/rules:defs.bzl", "CROSS_COMPILE_ERROR")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owned(target):
    """Returns the short paths a target owns as dart sources."""
    return sorted([f.short_path for f in target[QualitySourcesInfo].direct_sources["dart"].to_list()])

def _library_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, DartInfo in target, "dart_library keeps DartInfo")
    asserts.equals(env, ["dart/tests/lib/greet.dart"], _owned(target))
    return analysistest.end(env)

_library_test = analysistest.make(_library_test_impl)

def _binary_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.equals(env, "exe", target[DartCompileInfo].compile_mode)
    asserts.true(env, DartAnalyzableInfo in target, "dart_binary keeps DartAnalyzableInfo")
    asserts.equals(env, ["dart/tests/bin/greet.dart"], _owned(target))
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    asserts.true(env, "dart/tests/data/name.txt" in runfiles, "data reaches runfiles")
    asserts.true(env, target[DefaultInfo].files_to_run.executable != None, "dart_binary stays executable")
    return analysistest.end(env)

_binary_test = analysistest.make(_binary_test_impl)

def _test_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, DartAnalyzableInfo in target, "dart_test keeps DartAnalyzableInfo")
    asserts.equals(env, ["dart/tests/test/greet_test.dart"], _owned(target))
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    asserts.true(env, "dart/tests/data/name.txt" in runfiles, "data reaches test runfiles")
    return analysistest.end(env)

_test_test = analysistest.make(_test_test_impl)

def _cross_refused_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, CROSS_COMPILE_ERROR % ("linux", "riscv64"))
    return analysistest.end(env)

dart_cross_refused_test = analysistest.make(
    _cross_refused_test_impl,
    expect_failure = True,
    config_settings = {
        "//command_line_option:platforms": str(Label("//dart/tests:linux_riscv64")),
    },
)

def dart_rules_test_suite(name):
    """Declares the Dart wrapper analysis tests."""
    _library_test(name = name + "_library", target_under_test = ":greet")
    _binary_test(name = name + "_binary", target_under_test = ":greet_bin")
    _test_test(name = name + "_test", target_under_test = ":greet_test")
    native.test_suite(
        name = name,
        tests = [
            ":" + name + "_binary",
            ":" + name + "_library",
            ":" + name + "_test",
        ],
    )
