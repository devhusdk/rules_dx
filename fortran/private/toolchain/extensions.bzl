"""Module extension for fortran/private/toolchain."""

load("//core:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "FORTRAN_CELLS")
load(":repos.bzl", "FLANG_PREFIX", "FLANG_RT_NAME", "HUB_NAME", "flang_build", "flang_rt_repo", "hub_build")

def _fortran_impl(mctx):
    for cell in sorted(FORTRAN_CELLS.keys()):
        pinned_archive(
            name = FLANG_PREFIX + cell,
            files = {"BUILD.bazel": flang_build(cell)},
            sha256 = FORTRAN_CELLS[cell].sha256,
            strip_prefix = FORTRAN_CELLS[cell].strip_prefix,
            urls = FORTRAN_CELLS[cell].urls,
        )
    flang_rt_repo(name = FLANG_RT_NAME, build_file = Label("//fortran/private:flang_rt.BUILD.bazel"))
    hub_repo(name = HUB_NAME, files = hub_build())
    return mctx.extension_metadata(reproducible = True)

fortran = module_extension(
    implementation = _fortran_impl,
    doc = "Creates the pinned Flang compilers, flang-rt and the toolchain hub.",
)
