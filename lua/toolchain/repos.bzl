"""Hermetic LuaLS acquisition: one repository holding the release tree of the host cell."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("//libs/starlark:host.bzl", "host_cell")
load("//lua/coverage:pins.bzl", "LUACOV_BUILD", "LUACOV_PINS")
load(":pins.bzl", "LUALS_CELLS")

_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(["{binary}"])

filegroup(
    name = "tree",
    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"]),
)
"""

def luals_build(binary):
    """Returns the BUILD file of the LuaLS repository."""
    return _BUILD.format(binary = binary)

def _luals_impl(rctx):
    cell = host_cell(rctx)
    if cell == "windows_arm64":
        # LuaLS ships no Windows arm64 build; Windows on Arm runs the x64 one.
        cell = "windows_x86_64"
    pin = LUALS_CELLS.get(cell)
    if pin == None:
        fail("no pinned LuaLS archive for " + cell)
    rctx.download_and_extract(url = pin.urls, sha256 = pin.sha256)
    if not rctx.path(pin.binary).exists:
        fail("LuaLS archive " + pin.archive + " has no " + pin.binary)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", luals_build(pin.binary))

_luals = repository_rule(
    implementation = _luals_impl,
    doc = "Downloads the pinned LuaLS release archive of the host platform.",
)

def _lua_impl(mctx):
    _luals(name = "dx_luals")
    for pin in LUACOV_PINS:
        http_archive(
            name = pin.name,
            build_file_content = LUACOV_BUILD.format(root = "src/" if pin.name == "dx_luacov" else ""),
            sha256 = pin.sha256,
            strip_prefix = pin.strip_prefix,
            urls = pin.urls,
        )
    return mctx.extension_metadata(reproducible = True)

lua = module_extension(
    implementation = _lua_impl,
    doc = "Creates the pinned LuaLS, luacov and luacov-reporter-lcov repositories.",
)
