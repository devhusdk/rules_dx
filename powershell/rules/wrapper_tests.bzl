"""Wrapper conformance tests for PowerShell."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")

def powershell_wrapper_contract_tests(name):
    """Instantiates PowerShell wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["powershell"],
    )
