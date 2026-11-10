"""Hermetic LuaLS acquisition: one repository holding the release tree of the host cell."""

LUALS_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(["{binary}"])

filegroup(
    name = "tree",
    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"]),
)
"""
