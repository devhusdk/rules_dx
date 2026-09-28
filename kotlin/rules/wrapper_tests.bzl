"""Wrapper conformance tests for Kotlin."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "kotlin_kotlinc_opts_with_werror")

def kotlin_wrapper_contract_tests(name):
    """Instantiates Kotlin wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["kotlin"],
        extra_checks = [
            expect_equal(
                "empty gains warnings-as-errors",
                kotlin_kotlinc_opts_with_werror({})["kotlinc_opts"],
                "//kotlin/rules:warnings_as_errors",
            ),
            expect_equal(
                "explicit opts win",
                kotlin_kotlinc_opts_with_werror({"kotlinc_opts": "//custom:opts"})["kotlinc_opts"],
                "//custom:opts",
            ),
            expect_equal(
                "other kwargs survive",
                kotlin_kotlinc_opts_with_werror({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
