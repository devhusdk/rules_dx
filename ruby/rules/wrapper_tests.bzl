"""Wrapper conformance tests for Ruby."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")

def ruby_wrapper_contract_tests(name):
    """Instantiates Ruby wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["ruby"],
    )
