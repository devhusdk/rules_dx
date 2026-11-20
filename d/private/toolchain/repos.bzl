"""Hermetic LDC acquisition, the pinned rules_d sources and toolchain registration."""

load("//core:deb_closure.bzl", "deb_closure")
load("//core:jammy_runtime.bzl", "JAMMY_RUNTIME", "jammy_deb_urls")
load("//core:toolchain.bzl", "cell_repo", "pins_hub")
load("//qt/private/toolchain:seven_zip.bzl", "seven_zip_extract", "seven_zip_fetch")
load(":pins.bzl", "LDC_CELLS", "LDC_CROSS_CELLS", "LDC_VERSION", "MSVC_ABI")

HUB_NAME = "dx_d_toolchains"

CLOSURE_DIR = "dx_closure"

VERSION_FILE = "DX_LDC_VERSION"

CLOSURE_MARKER = CLOSURE_DIR + "/lib/DX_CLOSURE"

SEVEN_ZIP_DIR = "_dx_7zip"

SCRATCH_DIR = "_dx_extract"

ADDON_DIR = "_dx_addon"

def ldc_required_paths(cell):
    """Returns the paths one LDC cell's archive must provide."""
    return [cell.ldc, cell.compiler, "etc/ldc2.conf", cell.druntime, cell.phobos]

DMD_FILEGROUP = """filegroup(
    name = "dmd",
    srcs = ["{compiler}"],
    data = [":compiler_files"],
)"""

DMD_LAUNCHER = """dx_ldmd2(
    name = "dmd",
    closure_marker = {closure_marker},
    compiler = "{compiler}",
    data = [":compiler_files"],
)"""

LDC_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_import", "cc_library")
load("@rules_d//d:toolchain.bzl", "d_toolchain")
load("@rules_dx//d/private/toolchain:ldmd2.bzl", "dx_ldmd2")

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

{dmd}

filegroup(
    name = "dub",
    srcs = ["{dub}"],
)

d_toolchain(
    name = "toolchain",
    compiler_flags = {compiler_flags},
    d_compiler = ":dmd",
    druntime = ":druntime",
    dub_tool = ":dub",
    libphobos = ":libphobos",
    linker_flags = [],
)
"""

def ldc_repository_name(cell):
    """Returns the repository name one qualified cell's LDC is acquired into."""
    return cell_repo(HUB_NAME, cell)

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
    """Unpacks a .7z LDC archive with the pinned 7-Zip and drops the scratch archive."""
    seven_zip = seven_zip_fetch(rctx, SEVEN_ZIP_DIR)
    archive = SCRATCH_DIR + "/" + cell.archive
    rctx.download(url = cell.urls, output = archive, sha256 = cell.sha256)
    seven_zip_extract(rctx, seven_zip, archive, SCRATCH_DIR + "/out")
    rctx.delete(archive)
    for child in rctx.path(SCRATCH_DIR + "/out/" + cell.strip_prefix).readdir():
        rctx.rename(child, rctx.path(child.basename))
    rctx.delete(SCRATCH_DIR)

def _overlay_addon(rctx, addon):
    """Overlays the kept paths of an LDC addon package onto the repository root."""
    rctx.download_and_extract(url = addon.urls, sha256 = addon.sha256, output = ADDON_DIR)
    for path in addon.keep:
        rctx.rename(ADDON_DIR + "/" + path, path)
    rctx.delete(ADDON_DIR)

def _ldc_impl(rctx):
    cell = LDC_CELLS[rctx.attr.cell]
    if cell.archive.endswith(".7z"):
        _extract_7z(rctx, cell)
    else:
        rctx.download_and_extract(
            url = cell.urls,
            sha256 = cell.sha256,
            stripPrefix = cell.strip_prefix,
        )
    if cell.addon:
        _overlay_addon(rctx, cell.addon)
    for path in ldc_required_paths(cell):
        if not rctx.path(path).exists:
            fail("LDC archive " + cell.archive + " has no " + path)
    deb_closure(rctx, jammy_deb_urls, ldc_runtime(rctx.attr.cell), CLOSURE_DIR)
    if cell.wrapper and cell.runtime:
        rctx.file(CLOSURE_MARKER, "")
    rctx.file(VERSION_FILE, LDC_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", ldc_build(cell))
    return rctx.repo_metadata(reproducible = True)

def ldc_build(cell):
    """Renders the BUILD file of one LDC cell."""
    if cell.wrapper:
        dmd = DMD_LAUNCHER.format(
            compiler = cell.compiler,
            closure_marker = repr(CLOSURE_MARKER) if cell.runtime else "None",
        )
    else:
        dmd = DMD_FILEGROUP.format(compiler = cell.compiler)
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
        dmd = dmd,
    )

ldc_repo = repository_rule(
    implementation = _ldc_impl,
    doc = "Acquires one pinned LDC release plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
    },
)

CROSS_BUILD = """load("@rules_d//d:toolchain.bzl", "d_toolchain")

package(default_visibility = ["//visibility:public"])

d_toolchain(
    name = "toolchain",
    compiler_flags = {compiler_flags},
    d_compiler = "@{host}//:dmd",
    druntime = "@{target}//:druntime",
    dub_tool = "@{host}//:dub",
    libphobos = "@{target}//:libphobos",
    linker_flags = [],
)
"""

def cross_build(cross):
    """Renders the BUILD file of one cross cell."""
    target_flags = [flag for flag in LDC_CELLS[cross.target].compiler_flags if not flag.startswith("-mtriple=")]
    return CROSS_BUILD.format(
        compiler_flags = repr(target_flags + ["-mtriple=" + cross.triple]),
        host = ldc_repository_name(cross.host),
        target = ldc_repository_name(cross.target),
    )

def hub_build():
    """Returns the hub BUILD file: one D toolchain per qualified cell and per cross cell."""
    cross_constraints = {
        name: struct(
            exec_compatible_with = LDC_CELLS[cross.host].exec_compatible_with,
            target_compatible_with = LDC_CELLS[cross.target].exec_compatible_with,
        )
        for name, cross in LDC_CROSS_CELLS.items()
    }
    return pins_hub(
        HUB_NAME,
        list(LDC_CELLS.keys()) + list(LDC_CROSS_CELLS.keys()),
        str(Label("//d/private/toolchain:toolchain_type")),
        "both",
        os_constraints = {"windows": [MSVC_ABI]},
        cell_constraints = cross_constraints,
    )
