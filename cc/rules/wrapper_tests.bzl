"""Wrapper conformance tests for C/C++."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")

def cc_wrapper_contract_tests(name):
    """Instantiates C/C++ wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["c", "cpp", "cuda"],
    )
