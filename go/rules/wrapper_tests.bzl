"""Wrapper conformance tests for Go."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "go_effective_srcs")

def go_wrapper_contract_tests(name):
    """Instantiates Go wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["go"],
        extra_checks = [
            expect_equal(
                "none srcs become empty thin shape",
                go_effective_srcs(None),
                [],
            ),
            expect_equal(
                "empty srcs stay empty",
                go_effective_srcs([]),
                [],
            ),
            expect_equal(
                "sources are kept",
                go_effective_srcs(["hello.go"]),
                ["hello.go"],
            ),
        ],
    )
