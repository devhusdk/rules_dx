"""Wrapper conformance tests for the Qt rules."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "QT_SOURCE_SPECS")

def qt_wrapper_contract_tests(name):
    """Instantiates Qt wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = [file_class for file_class, _ in QT_SOURCE_SPECS],
        extra_checks = [
            expect_equal(
                "qt rules own c, cpp and qml sources",
                [file_class for file_class, _ in QT_SOURCE_SPECS],
                ["c", "cpp", "qml"],
            ),
        ],
    )
