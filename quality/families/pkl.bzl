"""Pkl real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook")
load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "pkl": {
        "attr": attr.label(default = "//config:tool_pkl", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "typecheck"],
        "kind": "file",
        "resources": "medium",
        "shard": "pkl",
    },
}

SHARD_SPECS = {
    "real_pkl_format": {
        "capability": "format",
        "doc": "Additive Pkl format family aspect (pkl format).",
        "shard": "pkl",
        "suffix": "-pkl",
    },
    "real_pkl_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Pkl typecheck family aspect (pkl eval, offline, one module per run).",
        "shard": "pkl",
        "suffix": "-pkl",
    },
}

HOOK_ORDER = ["pkl"]

HOOKS = {
    "pkl": default_files_hook(["pkl"], except_capability = "format"),
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_pkl_format_aspect = real_make_aspect(_CFG, "real_pkl_format")

real_pkl_typecheck_aspect = real_make_aspect(_CFG, "real_pkl_typecheck")
