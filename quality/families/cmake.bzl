"""Cmake real quality aspects."""

load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "cmakelint": {
        "attr": attr.label(default = "//quality/tools/python:cmakelint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "runfiles_env": True,
        "shard": "cmake",
    },
    "gersemi": {
        "attr": attr.label(default = "//config:tool_gersemi", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "cmake",
    },
}

SHARD_SPECS = {
    "real_cmake_format": {
        "capability": "format",
        "doc": "Additive CMake format family aspect (gersemi).",
        "shard": "cmake",
        "suffix": "-cmake",
    },
    "real_cmake_lint": {
        "capability": "lint",
        "doc": "Additive CMake lint family aspect (cmakelint).",
        "shard": "cmake",
        "suffix": "-cmake",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = [],
    context_hooks = {},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_cmake_format_aspect = real_make_aspect(_CFG, "real_cmake_format")

real_cmake_lint_aspect = real_make_aspect(_CFG, "real_cmake_lint")
