"""Wrapper-contract tests for the Python wrappers (item 2)."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "python_test_rejection", "python_unittest_test_rejection")

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
                "entrypoint; `main` is not supported. Use python_unittest_test " +
                "for unittest or python_py_test for a custom main.",
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
            expect_equal(
                "unittest main is rejected",
                python_unittest_test_rejection({"main": "main.py"}) != None,
                True,
            ),
            expect_equal(
                "unittest rejection names custom route",
                python_unittest_test_rejection({"main": "main.py"}),
                "python_unittest_test drives unittest and provides its own " +
                "entrypoint; `main` is not supported. Use python_py_test " +
                "for a custom main.",
            ),
            expect_equal(
                "unittest empty kwargs are clean",
                python_unittest_test_rejection({}),
                None,
            ),
            expect_equal(
                "unittest ordinary kwargs are clean",
                python_unittest_test_rejection({"deps": [":hello_lib"], "tags": ["small"]}),
                None,
            ),
        ],
    )
