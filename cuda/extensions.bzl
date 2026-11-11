"""Public CUDA module extensions."""

load("@rules_cuda//cuda:extensions.bzl", _toolchain = "toolchain")

toolchain = _toolchain
