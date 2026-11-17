"""Module extension for d/private/toolchain."""

load("//core:repo.bzl", "hub_repo")
load(":pins.bzl", "LDC_CELLS")
load(":repos.bzl", "HUB_NAME", "hub_build", "ldc_repo", "ldc_repository_name")

def _d_impl(mctx):
    for cell in sorted(LDC_CELLS.keys()):
        ldc_repo(name = ldc_repository_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    return mctx.extension_metadata(reproducible = True)

d = module_extension(
    implementation = _d_impl,
    doc = "Creates the pinned LDC compilers and the D toolchain hub.",
)
