"""Wrapper conformance tests for Scala."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "scala_scalacopts_with_werror")

def scala_wrapper_contract_tests(name):
    """Instantiates Scala wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["scala"],
        extra_checks = [
            expect_equal(
                "empty gains fatal warnings",
                scala_scalacopts_with_werror({})["scalacopts"],
                ["-Xfatal-warnings"],
            ),
            expect_equal(
                "existing flag is kept without duplication",
                scala_scalacopts_with_werror({"scalacopts": ["-Xfatal-warnings"]})["scalacopts"],
                ["-Xfatal-warnings"],
            ),
            expect_equal(
                "other flags are kept",
                scala_scalacopts_with_werror({"scalacopts": ["-deprecation"]})["scalacopts"],
                ["-deprecation", "-Xfatal-warnings"],
            ),
            expect_equal(
                "other kwargs survive",
                scala_scalacopts_with_werror({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
