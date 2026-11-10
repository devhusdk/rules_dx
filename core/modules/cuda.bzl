"""Pinned CUDA Toolkit components and the archs the pinned nvcc accepts."""

load("@dx_pins_data//:pins.bzl", "DATA")
load(":versions.bzl", _CUDA_REDIST_VERSION = "CUDA_REDIST_VERSION", _CUDA_VERSION = "CUDA_VERSION", _RULES_CUDA_VERSION = "RULES_CUDA_VERSION")

CUDA_VERSION = _CUDA_VERSION
CUDA_REDIST_VERSION = _CUDA_REDIST_VERSION
RULES_CUDA_VERSION = _RULES_CUDA_VERSION

# Newest clang major the pinned nvcc accepts as a host compiler (CUDA 13.4 Linux
# install guide, supported host compilers: https://docs.nvidia.com/cuda/cuda-installation-guide-linux/index.html).
# Recheck on every nvcc pin bump; once this reaches the main @llvm major, drop the
# nvcc_allow_unsupported_compiler feature from @rules_dx//cuda:defs.bzl.
CUDA_MAX_HOST_CLANG_MAJOR = 22

_PINS = DATA["dx_cuda"]

# The manifest rules_cuda's cuda.redist_json reads; every pin below must appear in it.
CUDA_REDIST_URL = _PINS["archives"]["redist_manifest"]["urls"][0]
CUDA_REDIST_SHA256 = _PINS["archives"]["redist_manifest"]["sha256"]

def _components(cell):
    return {
        name: struct(
            version = pin["version"],
            url = pin["urls"][0],
            sha256 = pin["sha256"],
            strip_prefix = pin["strip_prefix"],
        )
        for name, pin in _PINS["cells"][cell]["components"].items()
    }

# A cell with a blocker is pinned but not qualified.
CUDA_CELLS = {
    "linux_x86_64": struct(
        rules_cuda_platform = "linux-x86_64",
        blocker = "",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        components = _components("linux_x86_64"),
    ),
    "linux_arm64": struct(
        rules_cuda_platform = "linux-sbsa",
        blocker = "",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        components = _components("linux_arm64"),
    ),
    "windows_x86_64": struct(
        rules_cuda_platform = "windows-x86_64",
        blocker = "",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        components = _components("windows_x86_64"),
    ),
    "windows_arm64": struct(
        rules_cuda_platform = "windows-arm64",
        blocker = "",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        components = _components("windows_arm64"),
    ),
}

def cuda_qualified_cells():
    return [cell for cell, pin in CUDA_CELLS.items() if not pin.blocker]

CUDA_COMPONENTS = CUDA_CELLS["linux_x86_64"].components

CUDA_ARCHS = [
    "compute_75",
    "compute_80",
    "compute_86",
    "compute_87",
    "compute_89",
    "compute_90",
    "compute_100",
    "compute_103",
    "compute_120",
    "compute_121",
    "sm_75",
    "sm_80",
    "sm_86",
    "sm_87",
    "sm_89",
    "sm_90",
    "sm_100",
    "sm_103",
    "sm_120",
    "sm_121",
]
