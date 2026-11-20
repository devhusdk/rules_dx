"""The cells dx CUDA targets build for."""

load("//core:platform_support.bzl", "cells_support", "lang_cells")
load("//core/modules:cuda.bzl", "CUDA_CELLS")

PLATFORM_SUPPORT = cells_support("//cuda/private:defs.bzl", "CUDA_COMPATIBLE_WITH", CUDA_CELLS, lang_cells("cuda"), "//cuda/private/toolchain", unsupported = False)
