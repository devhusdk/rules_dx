"""Pinned CUDA Toolkit components and the archs the pinned nvcc accepts."""

load(":versions.bzl", _CUDA_HOST_LLVM_VERSION = "CUDA_HOST_LLVM_VERSION", _CUDA_VERSION = "CUDA_VERSION", _RULES_CUDA_VERSION = "FLOOR_RULES_CUDA", _TOOLCHAINS_LLVM_VERSION = "TOOLCHAINS_LLVM_VERSION")

CUDA_VERSION = _CUDA_VERSION
RULES_CUDA_VERSION = _RULES_CUDA_VERSION
TOOLCHAINS_LLVM_VERSION = _TOOLCHAINS_LLVM_VERSION
CUDA_HOST_LLVM_VERSION = _CUDA_HOST_LLVM_VERSION

_REDIST = "https://developer.download.nvidia.com/compute/cuda/redist/"

CUDA_COMPONENTS = {
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
