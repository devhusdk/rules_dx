"""The msix_tools module extension."""

load("//core:license.bzl", "accepted")
load("//core:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "BUILDTOOLS_SHA256", "BUILDTOOLS_URL", "LICENSE")
load(":repos.bzl", "HUB_NAME", "hub_build")

def _extension_impl(mctx):
    if accepted(mctx, LICENSE):
        pinned_archive(
            name = HUB_NAME,
            files = {"BUILD.bazel": hub_build(True)},
            sha256 = BUILDTOOLS_SHA256,
            type = "zip",
            urls = [BUILDTOOLS_URL],
        )
    else:
        hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(False)})
    return mctx.extension_metadata(reproducible = True)

msix_tools = module_extension(
    implementation = _extension_impl,
    doc = "Declares the pinned Windows SDK Build Tools repository.",
)
