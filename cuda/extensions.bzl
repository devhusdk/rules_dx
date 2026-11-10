"""Public CUDA module extensions."""

load("@rules_cuda//cuda:extensions.bzl", _toolchain = "toolchain")
load("//cuda/private/toolchain:extensions.bzl", _cuda_license = "cuda_license")

cuda_license = _cuda_license
toolchain = _toolchain
