"""D real quality aspects."""

load("//quality:compile_check.bzl", "compile_check_aspect")
load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "dfmt": {
        "attr": attr.label(default = "//config:tool_dfmt", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "d",
        "target_tool": True,
    },
    "dscanner": {
        "attr": attr.label(default = "//config:tool_dscanner", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "medium",
        "shard": "d",
        "target_tool": True,
    },
    "ldc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "d",
    },
}

SHARD_SPECS = {
    "real_d_typecheck": {
        "capability": "typecheck",
        "doc": "Additive D typecheck family aspect (ldc replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "d",
        "suffix": "-d",
    },
    "real_d_format": {
        "capability": "format",
        "doc": "Additive D format family aspect (dfmt).",
        "shard": "d",
        "suffix": "-d",
    },
    "real_d_lint": {
        "capability": "lint",
        "doc": "Additive D lint family aspect (D-Scanner).",
        "shard": "d",
        "suffix": "-d",
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

real_d_format_aspect = real_make_aspect(_CFG, "real_d_format")

real_d_lint_aspect = real_make_aspect(_CFG, "real_d_lint")

real_d_typecheck_aspect = real_make_aspect(_CFG, "real_d_typecheck")
