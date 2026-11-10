"""Module extension for the CUDA toolkit license."""

load("//core:license.bzl", "accepted", "license_tag")
load("//core:repo.bzl", "hub_repo")
load(":repos.bzl", "HUB_NAME", "LICENSE", "hub_build")


def _extension_impl(mctx):
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(accepted(mctx, "dx_cuda", LICENSE))})
    return mctx.extension_metadata(reproducible = True)


_license = license_tag("Accepts the CUDA Toolkit EULA for this build.")


cuda_license = module_extension(
    implementation = _extension_impl,
    doc = "Declares the CUDA Toolkit license repository.",
    tag_classes = {"license": _license},
)
