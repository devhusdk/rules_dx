"""Hermetic LuaLS acquisition: one repository holding the release tree of the host cell."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("//libs/starlark:toolchain.bzl", "host_archive")
load("//lua/coverage:pins.bzl", "LUACOV_BUILD", "LUACOV_PINS")
load(":pins.bzl", "LDOC_PINS", "LUALS_CELLS")

LUALS_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(["{binary}"])

filegroup(
    name = "tree",
    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"]),
)
"""

def _lua_impl(mctx):
    host_archive(
        name = "dx_luals",
        aliases = {"windows_arm64": "windows_x86_64"},
        build = LUALS_BUILD,
        cells = LUALS_CELLS,
        tool = "LuaLS",
    )
    for pin in LUACOV_PINS:
        http_archive(
            name = pin.name,
            build_file_content = LUACOV_BUILD.format(root = "src/" if pin.name == "dx_luacov" else ""),
            sha256 = pin.sha256,
            strip_prefix = pin.strip_prefix,
            urls = pin.urls,
        )
    for pin in LDOC_PINS:
        http_archive(
            name = pin.name,
            build_file_content = pin.build,
            sha256 = pin.sha256,
            strip_prefix = pin.strip_prefix,
            urls = pin.urls,
        )
    return mctx.extension_metadata(reproducible = True)

lua = module_extension(
    implementation = _lua_impl,
    doc = "Creates the pinned LuaLS, luacov, luacov-reporter-lcov, LDoc, Penlight and LuaFileSystem repositories.",
)
