"""Hermetic LDC acquisition, the pinned rules_d sources and toolchain registration."""

load("//erlang/toolchain:pins.bzl", "CELLS")
load("//erlang/toolchain:repos.bzl", "CLOSURE_DIR", "runtime_closure")
load(":pins.bzl", "DFMT_VERSION", "LDC_CELLS", "LDC_RELEASES", "LDC_VERSION", "RULES_D_SHA256", "RULES_D_URL", "RULES_D_VERSION", "TOOL_SOURCES")

HUB_NAME = "dx_d_toolchains"

LDC_PREFIX = "dx_d_ldc_"

VERSION_FILE = "DX_LDC_VERSION"

COMPILER_WRAPPER = "bin/dx_ldmd2"

REQUIRED_PATHS = [
    "bin/ldc2",
    "bin/ldmd2",
    "etc/ldc2.conf",
    "lib/libdruntime-ldc.a",
    "lib/libphobos2-ldc.a",
]

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
    "WORKSPACE",
    "WORKSPACE.bazel",
    "_typos.toml",
    "d/extensions.bzl",
    "d/private/toolchains_repo.bzl",
    "d/repositories.bzl",
    "d/tests",
    "docs",
    "dub",
    "e2e",
    "examples",
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

LDC_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_import")
load("@rules_d//d:toolchain.bzl", "d_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["{version_file}"])

_RUNTIME_LINKOPTS = [
    "-ldl",
    "-lm",
    "-lpthread",
    "-lrt",
]

filegroup(
    name = "compiler_files",
    srcs = [
        "bin/ldc2",
        "bin/ldmd2",
    ] + glob([
        "{closure}/lib/**",
        "etc/ldc2.conf/**",
        "import/**",
    ]),
)

cc_import(
    name = "druntime",
    linkopts = _RUNTIME_LINKOPTS,
    static_library = "lib/libdruntime-ldc.a",
)

cc_import(
    name = "libphobos",
    linkopts = _RUNTIME_LINKOPTS,
    static_library = "lib/libphobos2-ldc.a",
)

filegroup(
    name = "dmd",
    srcs = ["{wrapper}"],
    data = [":compiler_files"],
)

filegroup(
    name = "dub",
    srcs = ["bin/dub"],
)

d_toolchain(
    name = "d_toolchain",
    compiler_flags = [],
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

TOOL_BUILDS = {
    "dx_d_dfmt": _DFMT_BUILD,
    "dx_d_libdparse": _LIBDPARSE_BUILD,
}

_TOOLCHAIN = """toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:d_toolchain",
    toolchain_type = "@rules_d//d:toolchain_type",
)
"""

def ldc_repository_name(cell):
    """Returns the repository name one qualified cell's LDC is acquired into."""
    return LDC_PREFIX + cell

def ldc_runtime(cell):
    """Returns the pinned runtime closure packages one LDC cell needs."""
    wanted = LDC_CELLS[cell].runtime
    packages = [package for package in CELLS[cell].runtime if package.package in wanted]
    if len(packages) != len(wanted):
        fail("LDC cell " + cell + " names runtime packages the cell does not pin: " + repr(wanted))
    return packages

def _rules_d_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_d-" + rctx.attr.version,
    )
    for path in DROPPED_PATHS:
        rctx.delete(path)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _ROOT_BUILD)
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

_rules_d = repository_rule(
    implementation = _rules_d_impl,
    doc = "Fetches the pinned rules_d sources without their module, toolchain and dub setup.",
    attrs = {
        "patches": attr.label_list(doc = "Patches applied after extraction."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
        "version": attr.string(mandatory = True, doc = "Pinned rules_d release."),
    },
)

def _ldc_impl(rctx):
    cell = LDC_CELLS[rctx.attr.cell]
    rctx.download_and_extract(
        url = LDC_RELEASES + cell.archive,
        sha256 = cell.sha256,
        stripPrefix = cell.strip_prefix,
    )
    for path in REQUIRED_PATHS:
        if not rctx.path(path).exists:
            fail("LDC archive " + cell.archive + " has no " + path)
    runtime_closure(rctx, ldc_runtime(rctx.attr.cell))
    rctx.file(COMPILER_WRAPPER, WRAPPER.format(closure = CLOSURE_DIR), executable = True)
    rctx.file(VERSION_FILE, LDC_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", LDC_BUILD.format(
        closure = CLOSURE_DIR,
        version_file = VERSION_FILE,
        wrapper = COMPILER_WRAPPER,
    ))
    return rctx.repo_metadata(reproducible = True)

_ldc = repository_rule(
    implementation = _ldc_impl,
    doc = "Acquires one pinned LDC release plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
    },
)

def _hub_impl(rctx):
    content = []
    for cell in sorted(LDC_CELLS.keys()):
        content.append(_TOOLCHAIN.format(
            cell = cell,
            constraints = repr(CELLS[cell].exec_compatible_with),
            repo = ldc_repository_name(cell),
        ))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares one D toolchain per qualified cell.",
)

def _tool_source_impl(rctx):
    source = TOOL_SOURCES[rctx.attr.tool]
    rctx.download_and_extract(
        url = source.url,
        sha256 = source.sha256,
        stripPrefix = source.strip_prefix,
    )
    for path in source.dropped:
        rctx.delete(path)
    if rctx.attr.tool == "dx_d_dfmt":
        rctx.file("githash.txt", "v" + DFMT_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", TOOL_BUILDS[rctx.attr.tool])
    return rctx.repo_metadata(reproducible = True)

_tool_source = repository_rule(
    implementation = _tool_source_impl,
    doc = "Fetches the pinned sources of one D quality tool.",
    attrs = {
        "tool": attr.string(mandatory = True, doc = "Pinned tool source name."),
    },
)

def _d_impl(mctx):
    _rules_d(
        name = "rules_d",
        patches = [Label("//patches/rules_d:pic_link.patch")],
        sha256 = RULES_D_SHA256,
        url = RULES_D_URL,
        version = RULES_D_VERSION,
    )
    for cell in sorted(LDC_CELLS.keys()):
        _ldc(name = ldc_repository_name(cell), cell = cell)
    _hub(name = HUB_NAME)
    for tool in sorted(TOOL_SOURCES.keys()):
        _tool_source(name = tool, tool = tool)
    return mctx.extension_metadata(reproducible = True)

d = module_extension(
    implementation = _d_impl,
    doc = "Creates rules_d, the pinned LDC compilers and the D toolchain hub.",
)
