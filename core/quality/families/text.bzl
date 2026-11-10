"""Text real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "keep_sorted": {
        "attr": attr.label(default = "@dx_tools//:keep_sorted", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "text",
    },
    "typos": {
        "attr": attr.label(default = "@dx_tools//:typos", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "text",
    },
}

SHARD_SPECS = {
    "real_text_lint": {
        "capability": "lint",
        "doc": "Additive text lint family aspect (keep-sorted, typos).",
        "shard": "text",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_text_lint_aspect = real_make_aspect(_CFG, "real_text_lint")
