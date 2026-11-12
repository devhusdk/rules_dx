"""Tests for the CUDA EULA gate of the rules_cuda toolchain extension."""

load("//cuda:extensions.bzl", "cuda_component_repo_name", "cuda_license_error")
load("//core:defs.bzl", "expect_equal", "starlark_test")

def _module(is_root, *names):
    return struct(is_root = is_root, tags = struct(license = [struct(accept = [n]) for n in names]))

_UNACCEPTED = "dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add cuda.license(accept = [\"cuda-eula\"]) to the root MODULE.bazel"

def _checks():
    return [
        expect_equal("root accepts", cuda_license_error([_module(True, "cuda-eula")]), ""),
        expect_equal("nothing accepted", cuda_license_error([_module(True)]), _UNACCEPTED),
        expect_equal("only a non-root module accepts", cuda_license_error([_module(False, "cuda-eula"), _module(True)]), _UNACCEPTED),
        expect_equal("message text", cuda_license_error([]), "dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add cuda.license(accept = [\"cuda-eula\"]) to the root MODULE.bazel"),
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
