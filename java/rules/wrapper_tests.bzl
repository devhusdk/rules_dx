"""Wrapper conformance tests for Java."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "java_javacopts_with_werror")

def java_wrapper_contract_tests(name):
    """Instantiates Java wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["java"],
        extra_checks = [
            expect_equal(
                "empty gains werror and lint",
                java_javacopts_with_werror({})["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
            expect_equal(
                "existing werror is kept without duplication",
                java_javacopts_with_werror({"javacopts": ["-Werror"]})["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
            expect_equal(
                "existing lint gains werror",
                java_javacopts_with_werror({"javacopts": ["-Xlint:all"]})["javacopts"],
                ["-Xlint:all", "-Werror"],
            ),
            expect_equal(
                "both flags stay unchanged",
                java_javacopts_with_werror({"javacopts": ["-Werror", "-Xlint:all"]})["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
            expect_equal(
                "other kwargs survive",
                java_javacopts_with_werror({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
