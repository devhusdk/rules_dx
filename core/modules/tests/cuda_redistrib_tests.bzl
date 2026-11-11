"""The CUDA redistributable manifest lists every pinned component."""

load("//core:defs.bzl", "starlark_test")
load("//core/modules:cuda.bzl", _CUDA_CELLS = "CUDA_CELLS")

def cuda_redistrib_tests(name):
    """Fails when a pinned CUDA component is missing from the redistributable manifest.

    Args:
        name: Test target name.
    """
    redist_prefix = "https://developer.download.nvidia.com/compute/cuda/redist/"
    starlark_test(
        name = name,
        mode = "execution",
        file_blocks = {
            "@cuda_redistrib_json//file": [
                "            \"relative_path\": \"" + pin.url.removeprefix(redist_prefix) + "\",\n            \"sha256\": \"" + pin.sha256 + "\","
                for cell in _CUDA_CELLS.values()
                for pin in cell.components.values()
            ],
        },
    )
