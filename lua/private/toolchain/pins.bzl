"""Pinned LuaLS (lua-language-server) release archives."""

LUALS_VERSION = "3.19.2"

_RELEASE = "https://github.com/LuaLS/lua-language-server/releases/download/" + LUALS_VERSION + "/lua-language-server-" + LUALS_VERSION + "-"

# Keyed by the host cell of //core:host.bzl.
LUALS_CELLS = {
    "macos_arm64": struct(
        archive = "darwin-arm64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "79149584320a23bd5fcd5bc31f47a1975acd8a4560a0e89036bb1016fa61687f",
        urls = [_RELEASE + "darwin-arm64.tar.gz"],
    ),
    "macos_x86_64": struct(
        archive = "darwin-x64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "6f2174ece5ec63bcd9d5b22b2c902445c1cd9045fafe48c62602448ebf4c09cc",
        urls = [_RELEASE + "darwin-x64.tar.gz"],
    ),
    "linux_arm64": struct(
        archive = "linux-arm64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "17b8e401a874440adeb649e5024c364f2bb9d15156c74a40e56d1ae9a1b2e4f2",
        urls = [_RELEASE + "linux-arm64.tar.gz"],
    ),
    "linux_x86_64": struct(
        archive = "linux-x64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "93ad326b10f6c00911dd8c450fa098332dbd8f9f45d1052f5636a2d54479b947",
        urls = [_RELEASE + "linux-x64.tar.gz"],
    ),
    "windows_x86_64": struct(
        archive = "win32-x64.zip",
        binary = "bin/lua-language-server.exe",
        sha256 = "4a3dc851b3b31e7b75a23f645d1ba4a2137579a55f5fa4edbb17a3e2a6254044",
        urls = [_RELEASE + "win32-x64.zip"],
    ),
}

LDOC_VERSION = "1.5.0"

# LDoc runs on the pinned Lua. It needs Penlight and the C module LuaFileSystem, which
# tools/docs/lua:ldoc_main links in, so nothing is loaded from the host.
LDOC_PINS = [
    struct(
        name = "dx_ldoc",
        build = """package(default_visibility = ["//visibility:public"])

exports_files(["ldoc.lua"])

filegroup(
    name = "sources",
    srcs = glob(["ldoc.lua", "ldoc/**/*.lua"]),
)
""",
        sha256 = "4469cd74c8c7f51d3b9ce802d2239ba2b09d3d3a11273c3a5abdf273a0a53531",
        strip_prefix = "ldoc-" + LDOC_VERSION,
        urls = ["https://github.com/lunarmodules/LDoc/archive/refs/tags/v" + LDOC_VERSION + ".tar.gz"],
    ),
    struct(
        name = "dx_penlight",
        build = """package(default_visibility = ["//visibility:public"])

exports_files(["lua/pl/init.lua"])

filegroup(
    name = "sources",
    srcs = glob(["lua/**/*.lua"]),
)
""",
        sha256 = "2387431c0e83c4189cccb35b989141a3280d735cb5d42bacf3451af9869bebf7",
        strip_prefix = "Penlight-1.14.0",
        urls = ["https://github.com/lunarmodules/Penlight/archive/refs/tags/1.14.0.tar.gz"],
    ),
    struct(
        name = "dx_luafilesystem",
        build = """load("@rules_cc//cc:cc_library.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

cc_library(
    name = "lfs",
    srcs = [
        "src/lfs.c",
        "src/lfs.h",
    ],
    deps = ["@lua//:liblua"],
)
""",
        sha256 = "1142c1876e999b3e28d1c236bf21ffd9b023018e336ac25120fb5373aade1450",
        strip_prefix = "luafilesystem-1_9_0",
        urls = ["https://github.com/lunarmodules/luafilesystem/archive/refs/tags/v1_9_0.tar.gz"],
    ),
]
