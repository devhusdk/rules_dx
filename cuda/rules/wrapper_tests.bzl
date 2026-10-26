"""Wrapper conformance tests for CUDA."""

load("//cuda/toolchain:check.bzl", "arch_errors", "compiler_errors", "component_errors", "gpu_requirement_errors", "toolkit_errors")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_true")
load(":defs.bzl", "CUDA_TARGET", "cuda_gpu_env", "cuda_split_srcs", "cuda_target_compatible_with")

_SAXPY_ARCHS = ["compute_89:compute_89,sm_89", "sm_120"]

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
                [str(c) for c in cuda_target_compatible_with(None)],
                ["@@platforms//os:linux", "@@platforms//cpu:x86_64"],
            ),
            expect_equal(
                "extra constraints are kept once",
                [str(c) for c in cuda_target_compatible_with(["@platforms//os:linux", "//x:y"])],
                [str(c) for c in CUDA_TARGET] + ["//x:y"],
            ),
            expect_equal("pinned archs pass", arch_errors(["compute_89:compute_89,sm_89", "sm_120"]), []),
            expect_true("empty archs fail", "archs is empty" in arch_errors([])[0]),
            expect_true("sm_70 fails", "'sm_70' is not supported" in arch_errors(["sm_70"])[0]),
            expect_equal("pinned toolkit passes", toolkit_errors(_NVCC, "external/rules_cuda++toolchain+cuda_nvcc/nvcc", "13.4"), []),
            expect_equal("pinned component passes", component_errors("cublas", ["external/rules_cuda++toolchain+cuda_cublas/lib/libcublas.so.13"]), []),
            expect_true("local component fails", "not the pinned redistributable" in component_errors("cublas", ["/usr/lib/libcublas.so"])[0]),
            expect_true("other component fails", "not the pinned redistributable" in component_errors("cublas", ["external/rules_cuda++toolchain+cuda_cufft/lib/libcufft.so"])[0]),
            expect_true("empty component fails", "has no files" in component_errors("cublas", [])[0]),
            expect_true("clang fails", "dx CUDA targets compile with nvcc only" in compiler_errors("clang")[0]),
            expect_equal("nvcc passes", compiler_errors("nvcc"), []),
            expect_true("local toolkit fails", "not the pinned redistributable" in toolkit_errors("/usr/local/cuda/bin/nvcc", "/usr/local/cuda", "13.4")[0]),
            expect_true("other version fails", "dx CUDA targets need 13.4" in toolkit_errors(_NVCC, "", "12.9")[0]),
            expect_equal("gpu requirements pass", gpu_requirement_errors(_SAXPY_ARCHS, "8.9", "13.0"), []),
            expect_equal("gpu requirements at the pinned driver pass", gpu_requirement_errors(["sm_120"], "12.0", "13.4"), []),
            expect_true("cc below the lowest arch fails", "below the lowest arch in archs (8.9)" in gpu_requirement_errors(_SAXPY_ARCHS, "8.6", "13.0")[0]),
            expect_true("malformed cc fails", "min_compute_capability '89' is not X.Y" in gpu_requirement_errors(_SAXPY_ARCHS, "89", "13.0")[0]),
            expect_true("malformed driver fails", "min_driver '13' is not X.Y" in gpu_requirement_errors(_SAXPY_ARCHS, "8.9", "13")[0]),
            expect_true("driver above the pin fails", "min_driver 13.5 is outside 13.0 to 13.4" in gpu_requirement_errors(_SAXPY_ARCHS, "8.9", "13.5")[0]),
            expect_true("driver below the pin major fails", "min_driver 12.9 is outside 13.0 to 13.4" in gpu_requirement_errors(_SAXPY_ARCHS, "8.9", "12.9")[0]),
            expect_true("gpu errors carry the dx_cuda prefix", all([e.startswith("dx_cuda: ") for e in gpu_requirement_errors(["sm_89"], "x", "y")])),
            expect_equal(
                "gpu env pins the runtime",
                cuda_gpu_env("8.9", "13.0"),
                {"DX_GPU_MIN_CC": "8.9", "DX_GPU_MIN_DRIVER": "13.0", "DX_GPU_RUNTIME": "13.4"},
            ),
        ],
    )
