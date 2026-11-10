"""The config_setting each CUDA cell matches.

The labels are resolved here, in this module, so the cell helpers in @rules_dx never
look for them in their own repository.
"""

load("//core:platform_support.bzl", "cells_support")
load("//core/modules:cuda.bzl", "CUDA_CELLS", "cuda_qualified_cells")

CUDA_CELL_SETTINGS = {
    cell: Label("//cuda/private/toolchain:cell_%s_msvc" % cell if cell.startswith("windows_") else "//cuda/private/toolchain:cell_" + cell)
    for cell in CUDA_CELLS
}

PLATFORM_SUPPORT = cells_support("//cuda:defs.bzl", "CUDA_TARGET", CUDA_CELLS, cuda_qualified_cells(), "//cuda/private/toolchain", unsupported = False, settings = CUDA_CELL_SETTINGS)
