"""Module extension for fortran/toolchain."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "FORTRAN_CELLS", "RULES_FORTRAN_COMMIT", "RULES_FORTRAN_SHA256", "RULES_FORTRAN_URL")
load(":repos.bzl", "DROPPED_PATHS", "FLANG_PREFIX", "FLANG_RT_NAME", "HUB_NAME", "flang_build", "flang_rt_repo", "hub_build")


def _fortran_impl(mctx):
    pinned_archive(
        name = "rules_fortran",
        dropped = DROPPED_PATHS,
        patches = [
            Label("//patches/rules_fortran:module_map_key.patch"),
        ],
        sha256 = RULES_FORTRAN_SHA256,
        strip_prefix = "rules_fortran-" + RULES_FORTRAN_COMMIT,
        urls = [RULES_FORTRAN_URL],
    )
    for cell in sorted(FORTRAN_CELLS.keys()):
        pinned_archive(
            name = FLANG_PREFIX + cell,
            files = {"BUILD.bazel": flang_build(cell)},
            sha256 = FORTRAN_CELLS[cell].sha256,
            strip_prefix = FORTRAN_CELLS[cell].strip_prefix,
            urls = [FORTRAN_CELLS[cell].url],
        )
    flang_rt_repo(name = FLANG_RT_NAME, build_file = Label("//fortran/toolchain:flang_rt.BUILD.bazel"))
    hub_repo(name = HUB_NAME, files = hub_build())
    return mctx.extension_metadata(reproducible = True)


fortran = module_extension(
    implementation = _fortran_impl,
    doc = "Creates rules_fortran, the pinned Flang compilers, flang-rt and the toolchain hub.",
)
