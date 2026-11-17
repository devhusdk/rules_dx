"""Module extension for fortran/private/toolchain."""

load(":repos.bzl", "FLANG_RT_NAME", "flang_rt_repo")

def _fortran_impl(mctx):
    flang_rt_repo(name = FLANG_RT_NAME, build_file = Label("//fortran/private:flang_rt.BUILD.bazel"))
    return mctx.extension_metadata(reproducible = True)

fortran = module_extension(
    implementation = _fortran_impl,
    doc = "Creates the flang-rt sources built with the exec Flang of the dx_pins toolchain hub.",
)
