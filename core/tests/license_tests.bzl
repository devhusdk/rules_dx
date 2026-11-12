"""Shared license gate tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:license.bzl", "LICENSE_IDS", "unaccepted", "unknown_ids")

def _checks():
    return [
        expect_equal("known ids", unknown_ids(LICENSE_IDS), []),
        expect_equal("unknown id", unknown_ids(["cuda-eula", "nope"]), ["nope"]),
        expect_equal(
            "msix message",
            unaccepted("dx_msix", "Windows SDK Build Tools license", "https://aka.ms/WinSDKLicenseURL", "windows-sdk-buildtools"),
            "dx_msix: the Windows SDK Build Tools license (https://aka.ms/WinSDKLicenseURL) is not accepted; add dx_licenses.accept(ids = [\"windows-sdk-buildtools\"]) to the root MODULE.bazel",
        ),
    ]

def license_tests(name):
    """Declares the shared license gate test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
