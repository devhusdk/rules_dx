"""Module extension for d/private/toolchain."""

load("//core:repo.bzl", "hub_repo")
load(":pins.bzl", "LDC_CELLS", "LDC_CROSS_CELLS")
load(":repos.bzl", "HUB_NAME", "cross_build", "hub_build", "ldc_repo", "ldc_repository_name")

def _d_impl(mctx):
    for cell in sorted(LDC_CELLS.keys()):
        ldc_repo(name = ldc_repository_name(cell), cell = cell)
    for name, cross in sorted(LDC_CROSS_CELLS.items()):
        hub_repo(name = ldc_repository_name(name), files = {"BUILD.bazel": cross_build(cross)})
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    return mctx.extension_metadata(reproducible = True)

d = module_extension(
    implementation = _d_impl,
    doc = "Creates the pinned LDC compilers and the cross cells and the D toolchain hub.",
)
