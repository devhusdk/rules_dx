"""Makefile real quality aspects (checkmake and mbake)."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "checkmake": {
        "attr": attr.label(default = "@dx_tools//:checkmake", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "make",
    },
    "mbake": {
        "attr": attr.label(default = "//core/quality/tools/python:mbake", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "runfiles_env": True,
        "shard": "make",
    },
}

SHARD_SPECS = {
    "real_make_lint": {
        "capability": "lint",
        "doc": "Additive Makefile lint family aspect (checkmake).",
        "shard": "make",
        "suffix": "-make",
    },
    "real_make_format": {
        "capability": "format",
        "doc": "Additive Makefile format family aspect (mbake).",
        "shard": "make",
        "suffix": "-make",
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

real_make_lint_aspect = real_make_aspect(_CFG, "real_make_lint")

real_make_format_aspect = real_make_aspect(_CFG, "real_make_format")
