"""Wrapper conformance tests for F#."""

load("@rules_dotnet//dotnet/private:common.bzl", "framework_preprocessor_symbols")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "fsharp_tfm_with_defaults")
load(":symbols.bzl", "fsharp_compile_defines")

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
            expect_equal(
                "defines gain tfm symbols",
                fsharp_compile_defines("net10.0", ["net10.0"], ["X"]),
                framework_preprocessor_symbols("net10.0") + ["X"],
            ),
            expect_equal(
                "multi tfm uses highest compatible",
                fsharp_compile_defines("net10.0", ["net8.0", "net10.0"], []),
                framework_preprocessor_symbols("net10.0"),
            ),
            expect_equal(
                "older tfm symbols when only compatible",
                fsharp_compile_defines("net10.0", ["net8.0"], ["X"]),
                framework_preprocessor_symbols("net8.0") + ["X"],
            ),
            expect_equal(
                "no tfms keeps user defines",
                fsharp_compile_defines("net10.0", [], ["X"]),
                ["X"],
            ),
            expect_equal(
                "no defines gives tfm symbols",
                fsharp_compile_defines("net10.0", ["net10.0"], []),
                framework_preprocessor_symbols("net10.0"),
            ),
        ],
    )
