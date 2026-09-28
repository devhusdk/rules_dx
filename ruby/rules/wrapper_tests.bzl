"""Wrapper conformance tests for Ruby."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "ruby_effective_srcs")

def ruby_wrapper_contract_tests(name):
    """Instantiates Ruby wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["ruby"],
        extra_checks = [
            expect_equal(
                "none srcs become empty thin shape",
                ruby_effective_srcs(None),
                [],
            ),
            expect_equal(
                "empty srcs stay empty",
                ruby_effective_srcs([]),
                [],
            ),
            expect_equal(
                "sources are kept",
                ruby_effective_srcs(["hello.rb"]),
                ["hello.rb"],
            ),
        ],
    )
