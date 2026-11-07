"""CUDA toolkit license acceptance."""

load("@rules_dx//libs/starlark:license.bzl", "accepted", "license_tag", "unaccepted")
load("@rules_dx//libs/starlark:repo.bzl", "hub_repo")

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

def _extension_impl(mctx):
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(accepted(mctx, "dx_cuda", LICENSE))})
    return mctx.extension_metadata(reproducible = True)

_license = license_tag("Accepts the CUDA Toolkit EULA for this build.")

cuda_license = module_extension(
    implementation = _extension_impl,
    doc = "Declares the CUDA Toolkit license repository.",
    tag_classes = {"license": _license},
)
