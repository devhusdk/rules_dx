"""Wrapper conformance tests for C#."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "csharp_tfm_with_defaults")

def csharp_wrapper_contract_tests(name):
    """Instantiates C# wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["csharp"],
        extra_checks = [
            expect_equal(
                "empty gains net10 tfm",
                csharp_tfm_with_defaults({})["target_frameworks"],
                ["net10.0"],
            ),
            expect_equal(
                "empty gains warnings as errors",
                csharp_tfm_with_defaults({})["treat_warnings_as_errors"],
                True,
            ),
            expect_equal(
                "explicit tfm wins",
                csharp_tfm_with_defaults({"target_frameworks": ["net9.0"]})["target_frameworks"],
                ["net9.0"],
            ),
            expect_equal(
                "explicit tfm matrix wins",
                csharp_tfm_with_defaults({"target_frameworks": ["net8.0", "net10.0"]})["target_frameworks"],
                ["net8.0", "net10.0"],
            ),
            expect_equal(
                "explicit warnings setting wins",
                csharp_tfm_with_defaults({"treat_warnings_as_errors": False})["treat_warnings_as_errors"],
                False,
            ),
            expect_equal(
                "other kwargs survive",
                csharp_tfm_with_defaults({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
