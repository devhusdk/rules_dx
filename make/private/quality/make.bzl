"""Makefile real quality aspects (checkmake and mbake)."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "checkmake": {
        "attr": attr.label(default = "@dx_tools//:checkmake", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "make",
    },
    "mbake": {
        "attr": attr.label(default = "@dx_prebuilt//:python_tool_mbake", cfg = "exec", executable = True),
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
    },
    "real_make_format": {
        "capability": "format",
        "doc": "Additive Makefile format family aspect (mbake).",
        "shard": "make",
    },
}

HOOK_ORDER = []

HOOKS = {}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_make_lint_aspect = real_make_aspect(_CFG, "real_make_lint")

real_make_format_aspect = real_make_aspect(_CFG, "real_make_format")

MakeShardProbeInfo = provider(
    doc = "Shard names the make shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_make_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "make", MakeShardProbeInfo)

real_make_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "make", MakeShardProbeInfo)
