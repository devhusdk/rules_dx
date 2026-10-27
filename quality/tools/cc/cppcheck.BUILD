"""Cppcheck built from source with its cfg files next to the binary."""

load("@bazel_lib//lib:copy_to_bin.bzl", "copy_to_bin")
load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library")

package(default_visibility = ["//visibility:public"])

_COPTS = select({
    "@platforms//os:windows": [],
    "//conditions:default": ["-w"],
})

cc_library(
    name = "picojson",
    hdrs = glob(["externals/picojson/*.h"]),
    includes = ["externals/picojson"],
)

cc_library(
    name = "simplecpp",
    srcs = glob(["externals/simplecpp/*.cpp"]),
    hdrs = glob(["externals/simplecpp/*.h"]),
    copts = _COPTS,
    defines = select({
        "@platforms//os:windows": ["_WIN32_WINNT=0x0602"],
        "//conditions:default": [],
    }),
    includes = ["externals/simplecpp"],
)

cc_library(
    name = "tinyxml2",
    srcs = glob(["externals/tinyxml2/*.cpp"]),
    hdrs = glob(["externals/tinyxml2/*.h"]),
    copts = _COPTS,
    includes = ["externals/tinyxml2"],
)

cc_library(
    name = "core",
    srcs = glob(["lib/*.cpp"]),
    hdrs = glob(["lib/*.h"]),
    copts = _COPTS,
    strip_include_prefix = "lib",
    deps = [
        ":picojson",
        ":simplecpp",
        ":tinyxml2",
    ],
)

cc_library(
    name = "frontend",
    srcs = glob(["frontend/*.cpp"]),
    hdrs = glob(["frontend/*.h"]),
    copts = _COPTS,
    strip_include_prefix = "frontend",
    deps = [":core"],
)

cc_library(
    name = "cli",
    srcs = glob(
        ["cli/*.cpp"],
        exclude = ["cli/main.cpp"],
    ),
    hdrs = glob(["cli/*.h"]),
    copts = _COPTS,
    strip_include_prefix = "cli",
    deps = [
        ":core",
        ":frontend",
    ],
)

copy_to_bin(
    name = "cppcheck_cfg",
    srcs = glob(["cfg/*.cfg"]),
)

cc_binary(
    name = "cppcheck",
    srcs = ["cli/main.cpp"],
    copts = _COPTS,
    data = [":cppcheck_cfg"],
    linkopts = select({
        "@platforms//os:windows": ["-lshlwapi"],
        "//conditions:default": ["-lpthread"],
    }),
    deps = [
        ":cli",
        ":core",
    ],
)
