"""Tests for the CUDA EULA gate of the rules_cuda toolchain extension."""

load("//cuda:extensions.bzl", "cuda_component_repo_name", "cuda_license_error")
load("//core:defs.bzl", "expect_equal", "starlark_test")

_UNACCEPTED = "dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add dx_licenses.accept(ids = [\"cuda-eula\"]) to the root MODULE.bazel"

def _checks():
    return [
        expect_equal("accepted", cuda_license_error(["cuda-eula"]), ""),
        expect_equal("nothing accepted", cuda_license_error([]), _UNACCEPTED),
        expect_equal("other id accepted", cuda_license_error(["android-sdk-license"]), _UNACCEPTED),
        expect_equal("stub repo keeps the component repo name", cuda_component_repo_name("nvcc", "windows-arm64", "13.4.2"), "cuda_nvcc_windows_arm64_13_4_2"),
    ]

def cuda_license_gate_tests(name, size):
    """Declares the CUDA EULA gate test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
