"""Pinned CUDA Toolkit components and the archs the pinned nvcc accepts."""

load(":versions.bzl", _CUDA_HOST_LLVM_VERSION = "CUDA_HOST_LLVM_VERSION", _CUDA_VERSION = "CUDA_VERSION", _RULES_CUDA_VERSION = "RULES_CUDA_VERSION", _TOOLCHAINS_LLVM_VERSION = "TOOLCHAINS_LLVM_VERSION")

CUDA_VERSION = _CUDA_VERSION
RULES_CUDA_VERSION = _RULES_CUDA_VERSION
TOOLCHAINS_LLVM_VERSION = _TOOLCHAINS_LLVM_VERSION
CUDA_HOST_LLVM_VERSION = _CUDA_HOST_LLVM_VERSION

# Newest clang major the pinned nvcc accepts as a host compiler (CUDA 13.4 Linux
# install guide, supported host compilers: https://docs.nvidia.com/cuda/cuda-installation-guide-linux/index.html).
# Recheck on every nvcc pin bump; #3316 can only drop cuda_host_llvm once this is
# at least the main @llvm LLVM major.
CUDA_MAX_HOST_CLANG_MAJOR = 22

_REDIST = "https://developer.download.nvidia.com/compute/cuda/redist/"

_LINUX_X86_64_COMPONENTS = {
    "cccl": struct(
        version = "13.3.4.3.1",
        url = _REDIST + "cccl/linux-x86_64/cccl-linux-x86_64-13.3.4.3.1-archive.tar.xz",
        sha256 = "6b7516074f42f80dd3feb9c11318c8e1c5ab56cdd9f18fff4c5ace2e7e0a8209",
        strip_prefix = "cccl-linux-x86_64-13.3.4.3.1-archive",
    ),
    "crt": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_crt/linux-x86_64/cuda_crt-linux-x86_64-13.4.92-archive.tar.xz",
        sha256 = "c969e61ded12dbf0cf20ace72867f1622ec0e8ea1e1d305e27761012025f7aaa",
        strip_prefix = "cuda_crt-linux-x86_64-13.4.92-archive",
    ),
    "cublas": struct(
        version = "13.8.0.4",
        url = _REDIST + "libcublas/linux-x86_64/libcublas-linux-x86_64-13.8.0.4-archive.tar.xz",
        sha256 = "bd6ffcce561672da6abb928214a6694f392741e7d1e2d7dfde0da4bd651706b6",
        strip_prefix = "libcublas-linux-x86_64-13.8.0.4-archive",
    ),
    "cudart": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_cudart/linux-x86_64/cuda_cudart-linux-x86_64-13.4.92-archive.tar.xz",
        sha256 = "0ac5dbc538d04e9983bc493b410cce4b459e1ee9f5f6654b6464ef7b3e14a8b5",
        strip_prefix = "cuda_cudart-linux-x86_64-13.4.92-archive",
    ),
    "cufft": struct(
        version = "12.4.0.43",
        url = _REDIST + "libcufft/linux-x86_64/libcufft-linux-x86_64-12.4.0.43-archive.tar.xz",
        sha256 = "f7f8ac2cc67714989a3c8140cda39b1b7ed7d462a491e1ad3f53adb777f5070f",
        strip_prefix = "libcufft-linux-x86_64-12.4.0.43-archive",
    ),
    "curand": struct(
        version = "10.4.4.72",
        url = _REDIST + "libcurand/linux-x86_64/libcurand-linux-x86_64-10.4.4.72-archive.tar.xz",
        sha256 = "6f725f9187dc5f675308b830f786c163f5b1b499d86986f94fb60aa701c13c0b",
        strip_prefix = "libcurand-linux-x86_64-10.4.4.72-archive",
    ),
    "nvcc": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_nvcc/linux-x86_64/cuda_nvcc-linux-x86_64-13.4.92-archive.tar.xz",
        sha256 = "60998f40cc9df5826b2a846e2dcc328133fbc5ef539398ce0a21cf5dcac7193d",
        strip_prefix = "cuda_nvcc-linux-x86_64-13.4.92-archive",
    ),
    "nvvm": struct(
        version = "13.4.92",
        url = _REDIST + "libnvvm/linux-x86_64/libnvvm-linux-x86_64-13.4.92-archive.tar.xz",
        sha256 = "0e619cf3d5b27f7c812665599ca9761fc296e56129b24a85d822a5d8a1a3bd75",
        strip_prefix = "libnvvm-linux-x86_64-13.4.92-archive",
    ),
}

_WINDOWS_X86_64_COMPONENTS = {
    "cccl": struct(
        version = "13.3.4.3.1",
        url = _REDIST + "cccl/windows-x86_64/cccl-windows-x86_64-13.3.4.3.1-archive.zip",
        sha256 = "17d0ca1592c0dfc974a5fc39f470a6b7bae40da239e3e19bd0752017735acbca",
        strip_prefix = "cccl-windows-x86_64-13.3.4.3.1-archive",
    ),
    "crt": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_crt/windows-x86_64/cuda_crt-windows-x86_64-13.4.92-archive.zip",
        sha256 = "a38e3aa75e259128a1c7fb46e706182e4babca407cd3f3c447d21ab7fb14c7fd",
        strip_prefix = "cuda_crt-windows-x86_64-13.4.92-archive",
    ),
    "cublas": struct(
        version = "13.8.0.4",
        url = _REDIST + "libcublas/windows-x86_64/libcublas-windows-x86_64-13.8.0.4-archive.zip",
        sha256 = "0974318e9861a61cb9091cf7de8d4c2880e9787fe311cb4bcbd08865663c5f43",
        strip_prefix = "libcublas-windows-x86_64-13.8.0.4-archive",
    ),
    "cudart": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_cudart/windows-x86_64/cuda_cudart-windows-x86_64-13.4.92-archive.zip",
        sha256 = "621a70c4778287b1abf8c27c61b841d797fda2aeff01033374dd19624bcfa249",
        strip_prefix = "cuda_cudart-windows-x86_64-13.4.92-archive",
    ),
    "cufft": struct(
        version = "12.4.0.43",
        url = _REDIST + "libcufft/windows-x86_64/libcufft-windows-x86_64-12.4.0.43-archive.zip",
        sha256 = "69d0ad8dc3a1be66f01a748a8206d0ceaafa56939474663fbe790dc3e91d2009",
        strip_prefix = "libcufft-windows-x86_64-12.4.0.43-archive",
    ),
    "curand": struct(
        version = "10.4.4.72",
        url = _REDIST + "libcurand/windows-x86_64/libcurand-windows-x86_64-10.4.4.72-archive.zip",
        sha256 = "5f15b55cb83938b36a1f01aa9fb32715984e8495c2217081387a0c8ca836ff62",
        strip_prefix = "libcurand-windows-x86_64-10.4.4.72-archive",
    ),
    "nvcc": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_nvcc/windows-x86_64/cuda_nvcc-windows-x86_64-13.4.92-archive.zip",
        sha256 = "23950047aa5405993129a982a99924b4e6d5eb9bc312300030f3afd9b3b49ea4",
        strip_prefix = "cuda_nvcc-windows-x86_64-13.4.92-archive",
    ),
    "nvvm": struct(
        version = "13.4.92",
        url = _REDIST + "libnvvm/windows-x86_64/libnvvm-windows-x86_64-13.4.92-archive.zip",
        sha256 = "64fa39ad6332d6c40a02115d64827272f6befbc86e50568254f44d37d924e10b",
        strip_prefix = "libnvvm-windows-x86_64-13.4.92-archive",
    ),
}

_LINUX_ARM64_COMPONENTS = {
    "cccl": struct(
        version = "13.3.4.3.1",
        url = _REDIST + "cccl/linux-sbsa/cccl-linux-sbsa-13.3.4.3.1-archive.tar.xz",
        sha256 = "e36cc409c7ef4c3da18f0dd7f301b0df826121924cf817a7ff2ae508c944c6a1",
        strip_prefix = "cccl-linux-sbsa-13.3.4.3.1-archive",
    ),
    "crt": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_crt/linux-sbsa/cuda_crt-linux-sbsa-13.4.92-archive.tar.xz",
        sha256 = "0358db52257885611664240e030e45528f052aa1360c886f55dbaf2b2d61f87d",
        strip_prefix = "cuda_crt-linux-sbsa-13.4.92-archive",
    ),
    "cublas": struct(
        version = "13.8.0.4",
        url = _REDIST + "libcublas/linux-sbsa/libcublas-linux-sbsa-13.8.0.4-archive.tar.xz",
        sha256 = "34678808ea03c6fa57e62a3c8081dfa2e673da4c3838d1a6bef680a4ce22c88c",
        strip_prefix = "libcublas-linux-sbsa-13.8.0.4-archive",
    ),
    "cudart": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_cudart/linux-sbsa/cuda_cudart-linux-sbsa-13.4.92-archive.tar.xz",
        sha256 = "47e65e60d0f7cf832bff7992bd7a87ba4888195c31210392b5f23c607fe6c2a8",
        strip_prefix = "cuda_cudart-linux-sbsa-13.4.92-archive",
    ),
    "cufft": struct(
        version = "12.4.0.43",
        url = _REDIST + "libcufft/linux-sbsa/libcufft-linux-sbsa-12.4.0.43-archive.tar.xz",
        sha256 = "c62ea864ae7777e095463d10130d99e6d34995b996cda5f3d013c57bead7a574",
        strip_prefix = "libcufft-linux-sbsa-12.4.0.43-archive",
    ),
    "curand": struct(
        version = "10.4.4.72",
        url = _REDIST + "libcurand/linux-sbsa/libcurand-linux-sbsa-10.4.4.72-archive.tar.xz",
        sha256 = "2968e2d86dce64896ee3c2ac86c9e0f7decec4a0f157c8d73e3d0f31582e9e0c",
        strip_prefix = "libcurand-linux-sbsa-10.4.4.72-archive",
    ),
    "nvcc": struct(
        version = "13.4.92",
        url = _REDIST + "cuda_nvcc/linux-sbsa/cuda_nvcc-linux-sbsa-13.4.92-archive.tar.xz",
        sha256 = "8aebd451afb3984bc0019e803ae8a01a3837b7c69f36db779a7ce77e70831f5f",
        strip_prefix = "cuda_nvcc-linux-sbsa-13.4.92-archive",
    ),
    "nvvm": struct(
        version = "13.4.92",
        url = _REDIST + "libnvvm/linux-sbsa/libnvvm-linux-sbsa-13.4.92-archive.tar.xz",
        sha256 = "483085a4f52d272ea2c5a6cb4727b24efc2cecab4c4df99e80dfc6548e5b7929",
        strip_prefix = "libnvvm-linux-sbsa-13.4.92-archive",
    ),
}

# A cell with a blocker is pinned but not qualified.
CUDA_CELLS = {
    "linux_x86_64": struct(
        blocker = "",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        components = _LINUX_X86_64_COMPONENTS,
    ),
    "windows_x86_64": struct(
        blocker = "nvcc_msvc needs the MSVC cl.exe toolchain (#4815) and a Windows run (#4704)",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        components = _WINDOWS_X86_64_COMPONENTS,
    ),
    "linux_arm64": struct(
        blocker = "needs an aarch64 cuda_host_llvm sysroot (#5974), per-platform rules_cuda wiring (#5975) and an arm64 run (#3315)",
        redist = CUDA_VERSION,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        components = _LINUX_ARM64_COMPONENTS,
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

CUDA_HOST_SYSROOT = struct(
    url = "https://commondatastorage.googleapis.com/chrome-linux-sysroot/toolchain/2202c161310ffde63729f29d27fe7bb24a0bc540/debian_stretch_amd64_sysroot.tar.xz",
    sha256 = "84656a6df544ecef62169cfe3ab6e41bb4346a62d3ba2a045dc5a0a2ecea94a3",
)
