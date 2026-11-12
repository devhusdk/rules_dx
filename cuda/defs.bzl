"""Public CUDA API."""

load(
    "//cuda/private:defs.bzl",
    _cuda_binary = "cuda_binary",
    _cuda_component_library = "cuda_component_library",
    _cuda_library = "cuda_library",
    _cuda_objects = "cuda_objects",
    _cuda_test = "cuda_test",
)

cuda_binary = _cuda_binary
cuda_component_library = _cuda_component_library
cuda_library = _cuda_library
cuda_objects = _cuda_objects
cuda_test = _cuda_test
