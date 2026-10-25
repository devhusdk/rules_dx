"""Wrapper conformance tests for CUDA."""

load("//cuda/toolchain:check.bzl", "arch_errors", "compiler_errors", "toolkit_errors")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_true")
load(":defs.bzl", "CUDA_TARGET", "cuda_split_srcs", "cuda_target_compatible_with")

_NVCC = "external/rules_cuda++toolchain+cuda_nvcc/nvcc/bin/nvcc"

def cuda_wrapper_contract_tests(name):
    """Instantiates CUDA wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["cuda"],
        extra_checks = [
            expect_equal(
                "device and host sources split",
                cuda_split_srcs(["k.cu", "k.cuh", "main.cc", "api.h"]),
                (["k.cu", "k.cuh"], ["main.cc", "api.h"]),
            ),
            expect_equal(
                "every target is linux x86_64",
                cuda_target_compatible_with(None),
                CUDA_TARGET,
            ),
            expect_equal(
                "extra constraints are kept once",
                cuda_target_compatible_with(["@platforms//os:linux", "//x:y"]),
                CUDA_TARGET + ["//x:y"],
            ),
            expect_equal("pinned archs pass", arch_errors(["compute_89:compute_89,sm_89", "sm_120"]), []),
            expect_true("empty archs fail", "archs is empty" in arch_errors([])[0]),
            expect_true("sm_70 fails", "'sm_70' is not supported" in arch_errors(["sm_70"])[0]),
            expect_equal("pinned toolkit passes", toolkit_errors(_NVCC, "external/rules_cuda++toolchain+cuda_nvcc/nvcc", "13.4"), []),
            expect_true("clang fails", "dx CUDA targets compile with nvcc only" in compiler_errors("clang")[0]),
            expect_equal("nvcc passes", compiler_errors("nvcc"), []),
            expect_true("local toolkit fails", "not the pinned redistributable" in toolkit_errors("/usr/local/cuda/bin/nvcc", "/usr/local/cuda", "13.4")[0]),
            expect_true("other version fails", "dx CUDA targets need 13.4" in toolkit_errors(_NVCC, "", "12.9")[0]),
        ],
    )
