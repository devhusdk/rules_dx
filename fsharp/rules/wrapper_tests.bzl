"""Wrapper conformance tests for F#."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "fsharp_tfm_with_defaults")

def fsharp_wrapper_contract_tests(name):
    """Instantiates F# wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["fsharp"],
        extra_checks = [
            expect_equal(
                "empty gains net10 tfm",
                fsharp_tfm_with_defaults({})["target_frameworks"],
                ["net10.0"],
            ),
            expect_equal(
                "empty gains warnings as errors",
                fsharp_tfm_with_defaults({})["treat_warnings_as_errors"],
                True,
            ),
            expect_equal(
                "explicit tfm wins",
                fsharp_tfm_with_defaults({"target_frameworks": ["net9.0"]})["target_frameworks"],
                ["net9.0"],
            ),
            expect_equal(
                "explicit warnings setting wins",
                fsharp_tfm_with_defaults({"treat_warnings_as_errors": False})["treat_warnings_as_errors"],
                False,
            ),
            expect_equal(
                "other kwargs survive",
                fsharp_tfm_with_defaults({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
