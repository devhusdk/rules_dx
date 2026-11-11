"""Pinned LDoc, Penlight and LuaFileSystem sources; the LuaLS release archives are in pins.json."""

LDOC_VERSION = "1.5.0"

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
        sha256 = "b7a2c3e65799a649d29e9927a337f99cca2a5fc46367a0ce101435be74e53b39",
        strip_prefix = "Penlight-1.15.0",
        urls = ["https://github.com/lunarmodules/Penlight/archive/refs/tags/1.15.0.tar.gz"],
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
