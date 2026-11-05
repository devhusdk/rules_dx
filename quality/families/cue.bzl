"""CUE real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook")
load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "cue": {
        "attr": attr.label(default = "@dx_tools//:cue", allow_single_file = True, cfg = "exec"),
        "env": [["CUE_REGISTRY", "none"]],
        "kind": "file",
        "resources": "light",
        "shard": "cue",
    },
}

SHARD_SPECS = {
    "real_cue_format": {
        "capability": "format",
        "doc": "Additive CUE format family aspect (cue fmt).",
        "shard": "cue",
        "suffix": "-cue",
    },
    "real_cue_typecheck": {
        "capability": "typecheck",
        "doc": "Additive CUE typecheck family aspect (cue vet -c=false).",
        "shard": "cue",
        "suffix": "-cue",
    },
}

HOOK_ORDER = ["cue"]

HOOKS = {
    "cue": default_files_hook(["cue"]),
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

real_cue_format_aspect = real_make_aspect(_CFG, "real_cue_format")

real_cue_typecheck_aspect = real_make_aspect(_CFG, "real_cue_typecheck")
