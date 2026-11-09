"""The config_setting each CUDA cell matches.

The labels are resolved here, in this module, so the cell helpers in @rules_dx never
look for them in their own repository.
"""

load("@rules_dx//modules:cuda.bzl", "CUDA_CELLS")

# nvcc on Windows only drives cl.exe, so the Windows cells also need the msvc compiler flag.
CUDA_CELL_SETTINGS = {
    cell: Label("//toolchain:cell_%s_msvc" % cell if cell.startswith("windows_") else "//toolchain:cell_" + cell)
    for cell in CUDA_CELLS
}
