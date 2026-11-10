"""Latex real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "latexindent": {
        "attr": attr.label(default = "@dx_tools//:latexindent", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "latex",
    },
    "chktex": {
        "attr": attr.label(default = "@dx_tools//:chktex", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "latex",
    },
}

SHARD_SPECS = {
    "real_latex_format": {
        "capability": "format",
        "doc": "Additive LaTeX format family aspect (latexindent).",
        "shard": "latex",
        "suffix": "-latex",
    },
    "real_latex_lint": {
        "capability": "lint",
        "doc": "Additive LaTeX lint family aspect (chktex).",
        "shard": "latex",
        "suffix": "-latex",
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

real_latex_format_aspect = real_make_aspect(_CFG, "real_latex_format")

real_latex_lint_aspect = real_make_aspect(_CFG, "real_latex_lint")
