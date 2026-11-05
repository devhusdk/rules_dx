"""Pinned luacov and luacov-reporter-lcov sources for Lua coverage under `bazel coverage`."""

LUACOV_PINS = [
    struct(
        name = "dx_luacov",
        sha256 = "791a4a93a21b7288b1555cda8b7ad1ee0540ea606bd5dcc1028e76eeaec0e3d3",
        strip_prefix = "luacov-0.17.0",
        urls = ["https://github.com/lunarmodules/luacov/archive/refs/tags/v0.17.0.tar.gz"],
    ),
    struct(
        name = "dx_luacov_reporter_lcov",
        sha256 = "e450b209e54eea225546e8bdc5bb10bb9ba2e6d26200cfd7818031f6ef3c4640",
        strip_prefix = "luacov-reporter-lcov-0.2",
        urls = ["https://github.com/daurnimator/luacov-reporter-lcov/archive/refs/tags/v0.2.tar.gz"],
    ),
]

LUACOV_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "sources",
    srcs = glob(["{root}**/*.lua"], exclude = ["**/spec/**", "**/html/**", "**/reporter/html.lua"]),
)
"""
