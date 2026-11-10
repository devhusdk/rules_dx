"""CUDA toolkit license acceptance."""

load("@rules_dx//core:license.bzl", "accepted", "unaccepted")
HUB_NAME = "dx_cuda_license"

LICENSE = "cuda-eula"

LICENSE_URL = "https://docs.nvidia.com/cuda/eula/index.html"

UNACCEPTED = unaccepted("dx_cuda", "CUDA Toolkit EULA", LICENSE_URL, "cuda_license", LICENSE)

def hub_build(accepted):
    """Returns the BUILD text of the license repository."""
    return 'load("{}", "cuda_license")\n\npackage(default_visibility = ["//visibility:public"])\n\ncuda_license(\n    name = "license",\n    error = {},\n)\n'.format(
        str(Label(":check.bzl")),
        json.encode("" if accepted else UNACCEPTED),
    )
