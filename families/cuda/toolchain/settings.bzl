"""The config_setting each CUDA cell matches.

The labels are resolved here, in this module, so the cell helpers in @rules_dx never
look for them in their own repository.
"""

load("@rules_dx//modules:cuda.bzl", "CUDA_CELLS")

# nvcc on Windows only drives cl.exe, so the Windows cell also needs the msvc compiler flag.
CUDA_CELL_SETTINGS = {
    cell: Label("//toolchain:cell_windows_x86_64_msvc" if cell == "windows_x86_64" else "//toolchain:cell_" + cell)
    for cell in CUDA_CELLS
}
