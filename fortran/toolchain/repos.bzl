"""Hermetic Flang acquisition and Fortran toolchain registration."""

load(
    ":pins.bzl",
    "FLANG_RT_PREFIX",
    "FLANG_RT_SHA256",
    "FLANG_RT_URL",
    "FLANG_VERSION",
    "FORTRAN_CELLS",
    "RULES_FORTRAN_COMMIT",
    "RULES_FORTRAN_SHA256",
    "RULES_FORTRAN_URL",
)

HUB_NAME = "dx_fortran_toolchains"

FLANG_PREFIX = "dx_fortran_flang_"

FLANG_RT_NAME = "dx_fortran_flang_rt"

DROPPED_PATHS = [
    ".bazelignore",
    ".bazelrc",
    ".bazelversion",
    ".bcr",
    ".github",
    ".gitignore",
    "MODULE.bazel",
    "docs",
    "examples",
    "test",
]

FLANG_RT_TREES = [
    "flang-rt",
    "flang/include/flang",
    "flang/lib/Decimal",
    "flang/module",
]

_FLANG_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(["bin/flang", "bin/llvm-ar"])

filegroup(
    name = "flang",
    srcs = ["bin/flang"],
)

filegroup(
    name = "llvm_ar",
    srcs = ["bin/llvm-ar"],
)

filegroup(
    name = "compiler_files",
    srcs = ["bin/flang", "bin/flang-{major}"] + glob(["include/flang/**"]),
)
"""

_TOOLCHAIN = """dx_fortran_toolchain(
    name = "{cell}_toolchain",
    archiver = "@{repo}//:llvm_ar",
    compiler = "@{repo}//:flang",
    compiler_files = "@{repo}//:compiler_files",
    runtime = "@{runtime}//:flang_rt",
)

toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = ":{cell}_toolchain",
    toolchain_type = "@rules_fortran//fortran:toolchain_type",
)

config_setting(
    name = "{cell}_exec",
    constraint_values = {constraints},
)
"""

_TOOL = """alias(
    name = "{name}",
    actual = select({cases}),
)
"""

def flang_major(version):
    """Returns the major version of one LLVM release."""
    return version.split(".")[0]

def flang_rt_build(template, hub):
    """Returns the flang-rt BUILD file wired to the hub's exec Flang."""
    return template.replace("{flang}", "@" + hub + "//:flang").replace("{compiler_files}", "@" + hub + "//:compiler_files")

def _rules_fortran_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_fortran-" + rctx.attr.commit,
    )
    for path in DROPPED_PATHS:
        rctx.delete(path)
    rctx.file("REPO.bazel", "")
    return rctx.repo_metadata(reproducible = True)

_rules_fortran = repository_rule(
    implementation = _rules_fortran_impl,
    doc = "Fetches the pinned rules_fortran sources.",
    attrs = {
        "commit": attr.string(mandatory = True, doc = "Pinned rules_fortran commit."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
    },
)

def _flang_impl(rctx):
    cell = FORTRAN_CELLS[rctx.attr.cell]
    rctx.download_and_extract(url = cell.url, sha256 = cell.sha256, stripPrefix = cell.strip_prefix)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _FLANG_BUILD.format(major = flang_major(FLANG_VERSION)))
    return rctx.repo_metadata(reproducible = True)

_flang = repository_rule(
    implementation = _flang_impl,
    doc = "Installs the pinned static Flang compiler of one cell.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Cell key in FORTRAN_CELLS."),
    },
)

def _flang_rt_impl(rctx):
    archive = "_src/llvm-project.tar.xz"
    rctx.download(url = FLANG_RT_URL, output = archive, sha256 = FLANG_RT_SHA256)
    for tree in FLANG_RT_TREES:
        rctx.extract(archive, output = tree, strip_prefix = FLANG_RT_PREFIX + "/" + tree)
    rctx.delete("_src")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", flang_rt_build(rctx.read(rctx.attr.build_file), HUB_NAME))
    return rctx.repo_metadata(reproducible = True)

_flang_rt = repository_rule(
    implementation = _flang_rt_impl,
    doc = "Extracts the pinned flang-rt sources and their Flang headers.",
    attrs = {
        "build_file": attr.label(mandatory = True, doc = "BUILD file for the flang-rt sources."),
    },
)

def _hub_impl(rctx):
    content = ['load("@rules_dx//fortran/toolchain:toolchain.bzl", "dx_fortran_toolchain")\n\npackage(default_visibility = ["//visibility:public"])\n']
    for cell in sorted(FORTRAN_CELLS.keys()):
        content.append(_TOOLCHAIN.format(
            cell = cell,
            constraints = repr(FORTRAN_CELLS[cell].constraints),
            repo = FLANG_PREFIX + cell,
            runtime = FLANG_RT_NAME,
        ))
    for tool in ["compiler_files", "flang"]:
        cases = {":" + cell + "_exec": "@" + FLANG_PREFIX + cell + "//:" + tool for cell in sorted(FORTRAN_CELLS.keys())}
        content.append(_TOOL.format(name = tool, cases = repr(cases)))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares one Fortran toolchain per cell and the exec Flang of the host cell.",
)

def _fortran_impl(mctx):
    _rules_fortran(
        name = "rules_fortran",
        commit = RULES_FORTRAN_COMMIT,
        sha256 = RULES_FORTRAN_SHA256,
        url = RULES_FORTRAN_URL,
    )
    for cell in sorted(FORTRAN_CELLS.keys()):
        _flang(name = FLANG_PREFIX + cell, cell = cell)
    _flang_rt(name = FLANG_RT_NAME, build_file = Label("//fortran/toolchain:flang_rt.BUILD.bazel"))
    _hub(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

fortran = module_extension(
    implementation = _fortran_impl,
    doc = "Creates rules_fortran, the pinned Flang compilers, flang-rt and the toolchain hub.",
)
