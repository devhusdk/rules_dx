"""Hermetic flang-rt source acquisition."""

load(":pins.bzl", "FLANG_RT_PREFIX", "FLANG_RT_SHA256", "FLANG_RT_URLS")

HUB_NAME = "dx_fortran_toolchains"

FLANG_RT_NAME = "dx_fortran_flang_rt"

FLANG_RT_TREES = [
    "flang-rt",
    "flang/include/flang",
    "flang/lib/Decimal",
    "flang/module",
]

def flang_major(version):
    """Returns the major version of one LLVM release."""
    return version.split(".")[0]

def flang_rt_build(template, hub):
    """Returns the flang-rt BUILD file wired to the hub's exec Flang."""
    return template.replace("{flang}", "@" + hub + "//:flang").replace("{compiler_files}", "@" + hub + "//:compiler_files")

def _flang_rt_impl(rctx):
    archive = "_src/llvm-project.tar.xz"
    rctx.download(url = FLANG_RT_URLS, output = archive, sha256 = FLANG_RT_SHA256)
    for tree in FLANG_RT_TREES:
        rctx.extract(archive, output = tree, strip_prefix = FLANG_RT_PREFIX + "/" + tree)
    rctx.delete("_src")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", flang_rt_build(rctx.read(rctx.attr.build_file), HUB_NAME))
    return rctx.repo_metadata(reproducible = True)

flang_rt_repo = repository_rule(
    implementation = _flang_rt_impl,
    doc = "Extracts the pinned flang-rt sources and their Flang headers.",
    attrs = {
        "build_file": attr.label(mandatory = True, doc = "BUILD file for the flang-rt sources."),
    },
)
