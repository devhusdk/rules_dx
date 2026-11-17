"""Test that the GCC fallback stub fails with the unsupported-target message."""

load("//core:failure_test.bzl", "failure_test")
load("//cc/private/toolchain:unavailable.bzl", "GCC_UNSUPPORTED")

def gcc_unsupported_tests(name):
    """Declares the test that the stub toolchain fails when used."""
    failure_test(
        name = name,
        target = "//cc/private/toolchain:gcc_unsupported_impl",
        expected_failure_substring = GCC_UNSUPPORTED,
    )
