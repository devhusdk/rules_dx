"""Test that the GCC fallback stub fails with the unsupported-target message."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":unavailable.bzl", "GCC_UNSUPPORTED")

def _impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, GCC_UNSUPPORTED)
    return analysistest.end(env)

_gcc_unsupported_test = analysistest.make(_impl, expect_failure = True)

def gcc_unsupported_tests(name):
    """Declares the test that the stub toolchain fails when used.

    Args:
        name: Test target name.
    """
    _gcc_unsupported_test(
        name = name,
        target_under_test = ":gcc_unsupported_impl",
    )
