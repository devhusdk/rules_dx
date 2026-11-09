"""Hermetic Flang acquisition and Fortran toolchain registration."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load("//libs/starlark:toolchain.bzl", "toolchain_target")
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

exports_files(["bin/flang{exe}", "bin/llvm-ar{exe}"])

filegroup(
    name = "flang",
    srcs = ["bin/flang{exe}"],
)

filegroup(
    name = "llvm_ar",
    srcs = ["bin/llvm-ar{exe}"],
)

filegroup(
    name = "compiler_files",
    srcs = ["bin/flang{exe}", "bin/flang-{major}{exe}"] + glob(["include/flang/**"]),
)
"""

_IMPL = """dx_fortran_toolchain(
    name = "{cell}_toolchain",
    archiver = "@{repo}//:llvm_ar",
    compiler = "@{repo}//:flang",
    compiler_files = "@{repo}//:compiler_files",
    runtime = "@{runtime}//:flang_rt",
)
"""

_CONFIG = """config_setting(
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

def flang_build(cell):
    """Returns the BUILD file of one cell's Flang repository."""
    return _FLANG_BUILD.format(major = flang_major(FLANG_VERSION), exe = getattr(FORTRAN_CELLS[cell], "exe", ""))

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

def hub_build():
    """Returns the hub files: the root toolchains and exec Flang aliases, and impl with the toolchain targets."""
    impl = ['load("@rules_dx//fortran/toolchain:toolchain.bzl", "dx_fortran_toolchain")\n\npackage(default_visibility = ["//visibility:public"])\n']
    root = ['package(default_visibility = ["//visibility:public"])\n']
    for cell in sorted(FORTRAN_CELLS.keys()):
        impl.append(_IMPL.format(
            cell = cell,
            repo = FLANG_PREFIX + cell,
            runtime = FLANG_RT_NAME,
        ))
        root.append(toolchain_target(
            name = cell,
            exec_compatible_with = FORTRAN_CELLS[cell].constraints,
            target_compatible_with = FORTRAN_CELLS[cell].constraints,
            toolchain = "//impl:{}_toolchain".format(cell),
            toolchain_type = str(Label("@rules_fortran//fortran:toolchain_type")),
        ))
        root.append(_CONFIG.format(cell = cell, constraints = repr(FORTRAN_CELLS[cell].constraints)))
    for tool in ["compiler_files", "flang"]:
        cases = {":" + cell + "_exec": "@" + FLANG_PREFIX + cell + "//:" + tool for cell in sorted(FORTRAN_CELLS.keys())}
        root.append(_TOOL.format(name = tool, cases = repr(cases)))
    return {"BUILD.bazel": "\n".join(root), "impl/BUILD.bazel": "\n".join(impl)}

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
    _flang_rt(name = FLANG_RT_NAME, build_file = Label("//fortran/toolchain:flang_rt.BUILD.bazel"))
    hub_repo(name = HUB_NAME, files = hub_build())
    return mctx.extension_metadata(reproducible = True)

fortran = module_extension(
    implementation = _fortran_impl,
    doc = "Creates rules_fortran, the pinned Flang compilers, flang-rt and the toolchain hub.",
)
