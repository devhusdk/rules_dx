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
                kotlin_kotlinc_opts_with_werror("x", {})["kotlinc_opts"],
                str(Label("//kotlin/rules:warnings_as_errors")),
            ),
            expect_equal(
                "explicit opts are wrapped",
                kotlin_kotlinc_opts_with_werror("x", {"kotlinc_opts": "//custom:opts"})["kotlinc_opts"],
                ":x_kotlinc_opts",
            ),
            expect_equal(
                "none opts gain warnings-as-errors",
                kotlin_kotlinc_opts_with_werror("x", {"kotlinc_opts": None})["kotlinc_opts"],
                str(Label("//kotlin/rules:warnings_as_errors")),
            ),
            expect_equal(
                "other kwargs survive",
                kotlin_kotlinc_opts_with_werror("x", {"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
