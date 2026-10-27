"""Wrapper conformance tests for Go."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")

def go_wrapper_contract_tests(name):
    """Instantiates Go wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["go", "c", "cpp"],
    )
