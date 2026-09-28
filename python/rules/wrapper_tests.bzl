"""Wrapper-contract tests for the Python wrappers (item 2)."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "python_test_rejection")

def python_wrapper_contract_tests(name):
    """Instantiates Python wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = False,
        extra_checks = [
            expect_equal(
                "generic main is rejected",
                python_test_rejection({"main": "main.py"}) != None,
                True,
            ),
            expect_equal(
                "main rejection names pytest",
                python_test_rejection({"main": "main.py"}),
                "python_test always runs pytest and provides its own " +
                "entrypoint; `main` is not supported (generic mains and " +
                "alternate test drivers are rejected per " +
                "docs/testing/generation.md). Use py_pytest_main + py_test " +
                "directly for a custom main.",
            ),
            expect_equal(
                "empty kwargs are clean",
                python_test_rejection({}),
                None,
            ),
            expect_equal(
                "ordinary test kwargs are clean",
                python_test_rejection({"deps": ["@pypi//pytest"], "tags": ["small"]}),
                None,
            ),
            expect_equal(
                "pytest-native entrypoint kwargs stay pytest",
                python_test_rejection({"pytest_args": ["-k foo"], "chdir": "pkg"}),
                None,
            ),
        ],
    )
