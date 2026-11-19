"""Shared license gate tests."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load("//core:license.bzl", "CUDA_EULA", "LICENSE_IDS", "accept_line", "accepted_terms", "terms_errors", "unaccepted", "unknown_ids")

_CURRENT = "1" * 64
_OLD = "0" * 64

_TERMS = {CUDA_EULA: {"terms_sha256": _CURRENT, "url": "https://docs.nvidia.com/cuda/eula/index.html"}}

def _checks():
    return [
        expect_equal("known ids", unknown_ids(LICENSE_IDS), []),
        expect_equal("unknown id", unknown_ids(["cuda-eula", "nope"]), ["nope"]),
        expect_equal(
            "accept line pins the current terms",
            accept_line(_TERMS, [CUDA_EULA]),
            'dx_licenses.accept(ids = ["cuda-eula"], terms_sha256 = {"cuda-eula": "' + _CURRENT + '"})',
        ),
        expect_equal("ids alone stay valid", terms_errors(_TERMS, [struct(ids = [CUDA_EULA], terms_sha256 = {})]), []),
        expect_equal("current pin passes", terms_errors(_TERMS, [struct(ids = [CUDA_EULA], terms_sha256 = {CUDA_EULA: _CURRENT})]), []),
        expect_equal(
            "old pin asks to re-accept",
            terms_errors(_TERMS, [struct(ids = [CUDA_EULA], terms_sha256 = {CUDA_EULA: _OLD})]),
            ["dx_licenses: terms of cuda-eula changed: https://docs.nvidia.com/cuda/eula/index.html; re-accept with " + accept_line(_TERMS, [CUDA_EULA])],
        ),
        expect_equal(
            "pin outside ids",
            terms_errors(_TERMS, [struct(ids = [], terms_sha256 = {CUDA_EULA: _CURRENT})]),
            ["dx_licenses: terms_sha256 names cuda-eula, which the same tag's ids do not list"],
        ),
        expect_equal("accepted.json records the hash", accepted_terms(_TERMS, [CUDA_EULA]), {CUDA_EULA: _CURRENT}),
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
