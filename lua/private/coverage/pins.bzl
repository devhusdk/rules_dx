"""Pinned luacov and luacov-reporter-lcov sources for Lua coverage under `bazel coverage`; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_ARCHIVES = DATA["dx_lua_coverage"]["archives"]

LUACOV_PINS = [
    struct(
        name = name,
        sha256 = _ARCHIVES[name]["sha256"],
        strip_prefix = _ARCHIVES[name]["strip_prefix"],
        urls = _ARCHIVES[name]["urls"],
    )
    for name in ["dx_luacov", "dx_luacov_reporter_lcov"]
]

LUACOV_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "sources",
    srcs = glob(["{root}**/*.lua"], exclude = ["**/spec/**", "**/html/**", "**/reporter/html.lua"]),
)
"""
