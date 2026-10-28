"""Cross-compile refusal analysis tests for the Dart wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//dart/rules:defs.bzl", "CROSS_COMPILE_ERROR")

def _cross_refused_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, CROSS_COMPILE_ERROR % ("linux", "riscv64"))
    return analysistest.end(env)

dart_cross_refused_test = analysistest.make(
    _cross_refused_test_impl,
    expect_failure = True,
    config_settings = {
        "//command_line_option:platforms": str(Label("//dart/tests/cross:linux_riscv64")),
    },
)
