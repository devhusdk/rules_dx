"""Test that the GCC fallback stub fails with the unsupported-target message."""

load("//core:failure_test.bzl", "failure_test")
load(":unavailable.bzl", "GCC_UNSUPPORTED")

def gcc_unsupported_tests(name):
    """Declares the test that the stub toolchain fails when used.

    Args:
        name: Test target name.
    """
    failure_test(
        name = name,
        target = ":gcc_unsupported_impl",
        expected_failure_substring = GCC_UNSUPPORTED,
    )
