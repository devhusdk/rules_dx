"""Public CUDA API."""

load(
    "//cuda/private:defs.bzl",
    _CUDA_TARGET = "CUDA_TARGET",
    _cuda_binary = "cuda_binary",
    _cuda_component_library = "cuda_component_library",
    _cuda_gpu_env = "cuda_gpu_env",
    _cuda_library = "cuda_library",
    _cuda_objects = "cuda_objects",
    _cuda_test = "cuda_test",
)

CUDA_TARGET = _CUDA_TARGET
cuda_binary = _cuda_binary
cuda_component_library = _cuda_component_library
cuda_gpu_env = _cuda_gpu_env
cuda_library = _cuda_library
cuda_objects = _cuda_objects
cuda_test = _cuda_test
