"""Shared license gate tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:license.bzl", "license_errors", "root_accepts", "unaccepted")

def _module(is_root, *names):
    return struct(is_root = is_root, tags = struct(license = [struct(accept = [n]) for n in names]))

def _checks():
    return [
        expect_equal("root tag is accepted", root_accepts([_module(True, "a")]), ["a"]),
        expect_equal("non-root tag is ignored", root_accepts([_module(False, "a"), _module(True, "b")]), ["b"]),
        expect_equal("root tags merge", root_accepts([_module(True, "a"), _module(True, "b")]), ["a", "b"]),
        expect_equal("no tags", root_accepts([_module(True)]), []),
        expect_equal("known license", license_errors("dx_x", "a", ["a"]), []),
        expect_equal("unknown license", license_errors("dx_x", "a", ["b"]), ["dx_x: unknown license 'b'; the only one is 'a'"]),
        expect_equal(
            "msix message",
            unaccepted("dx_msix", "Windows SDK Build Tools license", "https://aka.ms/WinSDKLicenseURL", "msix_tools", "windows-sdk-buildtools"),
            "dx_msix: the Windows SDK Build Tools license (https://aka.ms/WinSDKLicenseURL) is not accepted; add msix_tools.license(accept = [\"windows-sdk-buildtools\"]) to the root MODULE.bazel",
        ),
        expect_equal(
            "cuda message",
            unaccepted("dx_cuda", "CUDA Toolkit EULA", "https://docs.nvidia.com/cuda/eula/index.html", "cuda_license", "cuda-eula"),
            "dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add cuda_license.license(accept = [\"cuda-eula\"]) to the root MODULE.bazel",
        ),
    ]

def license_tests(name):
    """Declares the shared license gate test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
