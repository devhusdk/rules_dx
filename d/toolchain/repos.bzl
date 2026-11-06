"""Hermetic LDC acquisition, the pinned rules_d sources and toolchain registration."""

load("//libs/starlark:jammy_runtime.bzl", "JAMMY_RUNTIME", "JAMMY_RUNTIME_MIRROR")
load("//libs/starlark:deb_closure.bzl", "deb_closure")
load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load("//libs/starlark:seven_zip.bzl", "seven_zip_extract", "seven_zip_fetch")
load(":pins.bzl", "LDC_CELLS", "LDC_RELEASES", "LDC_VERSION", "RULES_D_SHA256", "RULES_D_URL", "RULES_D_VERSION", "TOOL_SOURCES")

HUB_NAME = "dx_d_toolchains"

CLOSURE_DIR = "dx_closure"

LDC_PREFIX = "dx_d_ldc_"

VERSION_FILE = "DX_LDC_VERSION"

COMPILER_WRAPPER = "bin/dx_ldmd2"

SEVEN_ZIP_DIR = "_dx_7zip"

SCRATCH_DIR = "_dx_extract"

ADDON_DIR = "_dx_addon"

def ldc_required_paths(cell):
    """Returns the paths one LDC cell's archive must provide."""
    return [cell.ldc, cell.compiler, "etc/ldc2.conf", cell.druntime, cell.phobos]

DROPPED_PATHS = [
    ".bazelignore",
    ".bazelrc",
    ".bazelversion",
    ".bcr",
    ".github",
    ".pre-commit-config.yaml",
    "MODULE.bazel",
    "MODULE.bazel.lock",
    "REPO.bazel",
    "WORKSPACE.bazel",
    "_typos.toml",
    "d/extensions.bzl",
    "d/private/toolchains_repo.bzl",
    "d/repositories.bzl",
    "d/tests",
    "docs",
    "dub",
    "e2e",
    "renovate.json",
    "tools",
]

_ROOT_BUILD = """exports_files(["LICENSE"])
"""

WRAPPER = """#!/bin/sh
bin="${{0%/*}}"
LD_LIBRARY_PATH="$bin/../{closure}/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"
export LD_LIBRARY_PATH
exec "$bin/ldmd2" "$@"
"""

LDC_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_import", "cc_library")
load("@rules_d//d:toolchain.bzl", "d_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["{version_file}"])

_RUNTIME_LINKOPTS = {linkopts}

filegroup(
    name = "compiler_files",
    srcs = [
        "{ldc}",
        "{compiler}",
    ] + glob([
{closure_glob}{data_globs}        "etc/ldc2.conf/**",
        "import/**",
    ]),
)

cc_import(
    name = "druntime_import",
    linkopts = _RUNTIME_LINKOPTS,
    static_library = "{druntime}",
)

cc_library(
    name = "druntime",
    deps = [":druntime_import"] + {runtime_deps},
)

cc_import(
    name = "libphobos",
    linkopts = _RUNTIME_LINKOPTS,
    static_library = "{phobos}",
)

filegroup(
    name = "dmd",
    srcs = ["{wrapper}"],
    data = [":compiler_files"],
)

filegroup(
    name = "dub",
    srcs = ["{dub}"],
)

d_toolchain(
    name = "d_toolchain",
    compiler_flags = {compiler_flags},
    d_compiler = ":dmd",
    druntime = ":druntime",
    dub_tool = ":dub",
    libphobos = ":libphobos",
    linker_flags = [],
)
"""

_LIBDPARSE_BUILD = """load("@rules_d//d:defs.bzl", "d_library")

package(default_visibility = ["//visibility:public"])

d_library(
    name = "dparse",
    srcs = glob(["src/**/*.d"]),
    imports = ["src"],
)
"""

_DFMT_BUILD = """load("@rules_d//d:defs.bzl", "d_binary")
load("@rules_dx//d/rules:defs.bzl", "D_COMPATIBLE_WITH")

package(default_visibility = ["//visibility:public"])

filegroup(
    name = "githash",
    srcs = ["githash.txt"],
)

d_binary(
    name = "dfmt",
    srcs = glob(["src/**/*.d"]),
    imports = ["src"],
    string_srcs = [":githash"],
    target_compatible_with = D_COMPATIBLE_WITH,
    deps = ["@dx_d_libdparse//:dparse"],
)
"""

_CONTAINERS_BUILD = """load("@rules_d//d:defs.bzl", "d_library")

package(default_visibility = ["//visibility:public"])

d_library(
    name = "containers",
    srcs = glob(["src/**/*.d"]),
    imports = ["src"],
)
"""

_INIFILED_BUILD = """load("@rules_d//d:defs.bzl", "d_library")

package(default_visibility = ["//visibility:public"])

d_library(
    name = "inifiled",
    srcs = glob(["source/**/*.d"]),
    imports = ["source"],
)
"""

_DDOC_BUILD = """load("@rules_d//d:defs.bzl", "d_library")

package(default_visibility = ["//visibility:public"])

d_library(
    name = "ddoc",
    srcs = glob([
        "src/**/*.d",
        "common/source/**/*.d",
    ]),
    imports = [
        "common/source",
        "src",
    ],
    deps = ["@dx_d_libdparse//:dparse"],
)
"""

_DSYMBOL_BUILD = """load("@rules_d//d:defs.bzl", "d_library")

package(default_visibility = ["//visibility:public"])

d_library(
    name = "dsymbol",
    srcs = glob(
        ["dsymbol/src/**/*.d"],
        exclude = ["dsymbol/src/dsymbol/tests.d"],
    ),
    imports = ["dsymbol/src"],
    deps = [
        "@dx_d_containers//:containers",
        "@dx_d_libdparse//:dparse",
    ],
)
"""

_DSCANNER_BUILD = """load("@rules_d//d:defs.bzl", "d_binary")
load("@rules_dx//d/rules:defs.bzl", "D_COMPATIBLE_WITH")

package(default_visibility = ["//visibility:public"])

filegroup(
    name = "githash",
    srcs = ["githash.txt"],
)

d_binary(
    name = "dscanner",
    srcs = glob(["src/**/*.d"]),
    imports = ["src"],
    string_srcs = [":githash"],
    target_compatible_with = D_COMPATIBLE_WITH,
    deps = [
        "@dx_d_containers//:containers",
        "@dx_d_dcd//:dsymbol",
        "@dx_d_inifiled//:inifiled",
        "@dx_d_libddoc//:ddoc",
        "@dx_d_libdparse//:dparse",
    ],
)
"""

TOOL_BUILDS = {
    "dx_d_containers": _CONTAINERS_BUILD,
    "dx_d_dcd": _DSYMBOL_BUILD,
    "dx_d_dfmt": _DFMT_BUILD,
    "dx_d_dscanner": _DSCANNER_BUILD,
    "dx_d_inifiled": _INIFILED_BUILD,
    "dx_d_libddoc": _DDOC_BUILD,
    "dx_d_libdparse": _LIBDPARSE_BUILD,
}

# Source roots of each tool repository. The D sources they hold are the corpus
# gazelle/d parses to catch grammar regressions.
TOOL_SRC_ROOTS = {
    "dx_d_containers": ["src"],
    "dx_d_dcd": ["dsymbol/src"],
    "dx_d_dfmt": ["src"],
    "dx_d_dscanner": ["src"],
    "dx_d_inifiled": ["source"],
    "dx_d_libddoc": ["common/source", "src"],
    "dx_d_libdparse": ["src"],
}

def tool_build(tool):
    """Returns the BUILD file of one tool repository: its build plus the d_srcs corpus filegroup."""
    globs = ", ".join(['"{}/**/*.d"'.format(root) for root in TOOL_SRC_ROOTS[tool]])
    return TOOL_BUILDS[tool] + """
filegroup(
    name = "d_srcs",
    srcs = glob([{}]),
)
""".format(globs)

_TOOLCHAIN = """toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:d_toolchain",
    toolchain_type = "{toolchain_type}",
)
"""

def ldc_repository_name(cell):
    """Returns the repository name one qualified cell's LDC is acquired into."""
    return LDC_PREFIX + cell

def ldc_runtime(cell):
    """Returns the pinned runtime closure packages one LDC cell needs."""
    wanted = LDC_CELLS[cell].runtime
    if not wanted:
        return []
    packages = [package for package in JAMMY_RUNTIME[cell] if package.package in wanted]
    if len(packages) != len(wanted):
        fail("LDC cell " + cell + " names runtime packages the cell does not pin: " + repr(wanted))
    return packages

def _extract_7z(rctx, cell):
    """Unpacks a .7z LDC archive with the pinned 7-Zip and drops the scratch archive.

    The extractor stays in SEVEN_ZIP_DIR: Windows arm64 keeps the emulated 7zr.exe locked, so deleting it fails the fetch.
    """
    seven_zip = seven_zip_fetch(rctx, SEVEN_ZIP_DIR)
    archive = SCRATCH_DIR + "/" + cell.archive
    rctx.download(url = LDC_RELEASES + cell.archive, output = archive, sha256 = cell.sha256)
    seven_zip_extract(rctx, seven_zip, archive, SCRATCH_DIR + "/out")
    rctx.delete(archive)
    for child in rctx.path(SCRATCH_DIR + "/out/" + cell.strip_prefix).readdir():
        rctx.rename(child, rctx.path(child.basename))
    rctx.delete(SCRATCH_DIR)

def _overlay_addon(rctx, addon):
    """Overlays the kept paths of an LDC addon package onto the repository root."""
    rctx.download_and_extract(url = LDC_RELEASES + addon.archive, sha256 = addon.sha256, output = ADDON_DIR)
    for path in addon.keep:
        rctx.rename(ADDON_DIR + "/" + path, path)
    rctx.delete(ADDON_DIR)

def _ldc_impl(rctx):
    cell = LDC_CELLS[rctx.attr.cell]
    if cell.archive.endswith(".7z"):
        _extract_7z(rctx, cell)
    else:
        rctx.download_and_extract(
            url = LDC_RELEASES + cell.archive,
            sha256 = cell.sha256,
            stripPrefix = cell.strip_prefix,
        )
    if cell.addon:
        _overlay_addon(rctx, cell.addon)
    for path in ldc_required_paths(cell):
        if not rctx.path(path).exists:
            fail("LDC archive " + cell.archive + " has no " + path)
    deb_closure(rctx, JAMMY_RUNTIME_MIRROR, ldc_runtime(rctx.attr.cell), CLOSURE_DIR)
    compiler = cell.compiler
    if cell.wrapper:
        compiler = COMPILER_WRAPPER
        rctx.file(COMPILER_WRAPPER, WRAPPER.format(closure = CLOSURE_DIR), executable = True)
    rctx.file(VERSION_FILE, LDC_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", ldc_build(cell, compiler))
    return rctx.repo_metadata(reproducible = True)

def ldc_build(cell, wrapper):
    """Renders the BUILD file of one LDC cell; wrapper is the d_compiler launcher or the compiler itself."""
    return LDC_BUILD.format(
        closure_glob = '        "' + CLOSURE_DIR + '/lib/**",\n' if cell.runtime else "",
        compiler = cell.compiler,
        compiler_flags = repr(cell.compiler_flags),
        data_globs = "".join(['        "' + glob + '",\n' for glob in cell.data_globs]),
        druntime = cell.druntime,
        dub = cell.dub,
        ldc = cell.ldc,
        linkopts = repr(cell.linkopts),
        phobos = cell.phobos,
        runtime_deps = repr([str(Label(dep)) for dep in cell.runtime_deps]),
        version_file = VERSION_FILE,
        wrapper = wrapper,
    )

_ldc = repository_rule(
    implementation = _ldc_impl,
    doc = "Acquires one pinned LDC release plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
    },
)

def hub_build():
    """Returns the hub BUILD file: one D toolchain per qualified cell."""
    content = []
    for cell in sorted(LDC_CELLS.keys()):
        content.append(_TOOLCHAIN.format(
            cell = cell,
            constraints = repr(LDC_CELLS[cell].exec_compatible_with),
            repo = ldc_repository_name(cell),
            toolchain_type = str(Label("//d/toolchain/type:toolchain_type")),
        ))
    return "\n".join(content)

def tool_files(tool):
    """Returns the files written into one D quality tool source repository."""
    files = dict(TOOL_SOURCES[tool].files)
    files["BUILD.bazel"] = tool_build(tool)
    return files

def _d_impl(mctx):
    pinned_archive(
        name = "rules_d",
        dropped = DROPPED_PATHS,
        files = {"BUILD.bazel": _ROOT_BUILD},
        patches = [
            Label("//patches/rules_d:pic_link.patch"),
            Label("//patches/rules_d:toolchain_type_alias.patch"),
        ],
        sha256 = RULES_D_SHA256,
        strip_prefix = "rules_d-" + RULES_D_VERSION,
        urls = [RULES_D_URL],
    )
    for cell in sorted(LDC_CELLS.keys()):
        _ldc(name = ldc_repository_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    for tool in sorted(TOOL_SOURCES.keys()):
        source = TOOL_SOURCES[tool]
        pinned_archive(
            name = tool,
            dropped = source.dropped,
            files = tool_files(tool),
            sha256 = source.sha256,
            strip_prefix = source.strip_prefix,
            urls = [source.url],
        )
    return mctx.extension_metadata(reproducible = True)

d = module_extension(
    implementation = _d_impl,
    doc = "Creates rules_d, the pinned LDC compilers and the D toolchain hub.",
)
