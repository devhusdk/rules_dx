"""Typst real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "tinymist": {
        "attr": attr.label(default = "@dx_tools//:tinymist", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "typst",
    },
    "typstyle": {
        "attr": attr.label(default = "@dx_tools//:typstyle", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "typst",
    },
}

SHARD_SPECS = {
    "real_typst_format": {
        "capability": "format",
        "doc": "Additive Typst format family aspect (typstyle).",
        "shard": "typst",
    },
    "real_typst_lint": {
        "capability": "lint",
        "doc": "Additive Typst lint family aspect (tinymist).",
        "shard": "typst",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_typst_format_aspect = real_make_aspect(_CFG, "real_typst_format")

real_typst_lint_aspect = real_make_aspect(_CFG, "real_typst_lint")
