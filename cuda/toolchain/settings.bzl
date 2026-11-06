"""Cell settings that need more than the host platform."""

# nvcc on Windows only drives cl.exe, so the Windows cell also needs the msvc compiler flag.
CUDA_CELL_SETTINGS = {
    "windows_x86_64": Label("//cuda/toolchain:cell_windows_x86_64_msvc"),
}
