"""The config_setting each CUDA cell matches.

The labels are resolved here, in this module, so the cell helpers in @rules_dx never
look for them in their own repository.
"""

load("//core:platform_support.bzl", "cells_support", "lang_cells")
load("//core/modules:cuda.bzl", "CUDA_CELLS")

CUDA_CELL_SETTINGS = {
    cell: Label("//cuda/private/toolchain:cell_%s_msvc" % cell if cell.startswith("windows_") else "//cuda/private/toolchain:cell_" + cell)
    for cell in CUDA_CELLS
}

PLATFORM_SUPPORT = cells_support("//cuda/private:defs.bzl", "CUDA_COMPATIBLE_WITH", CUDA_CELLS, lang_cells("cuda"), "//cuda/private/toolchain", unsupported = False, settings = CUDA_CELL_SETTINGS)
