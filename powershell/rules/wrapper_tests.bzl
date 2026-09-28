"""Wrapper conformance tests for PowerShell."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "powershell_effective_srcs")

def powershell_wrapper_contract_tests(name):
    """Instantiates PowerShell wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["powershell"],
        extra_checks = [
            expect_equal(
                "none srcs become empty thin shape",
                powershell_effective_srcs(None),
                [],
            ),
            expect_equal(
                "empty srcs stay empty",
                powershell_effective_srcs([]),
                [],
            ),
            expect_equal(
                "sources are kept",
                powershell_effective_srcs(["Hello.ps1"]),
                ["Hello.ps1"],
            ),
        ],
    )
