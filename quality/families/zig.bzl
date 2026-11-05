"""Zig real quality aspects."""

load("//quality:compile_check.bzl", "compile_check_aspect", "compile_check_hook")
load("//quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "zig": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "zig",
    },
    "zig_ast_check": {
        "attr": attr.label(default = "//config:tool_zig_ast_check", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "zig",
        "target_tool": True,
    },
    "zig_fmt": {
        "attr": attr.label(default = "//config:tool_zig_fmt", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "zig",
        "target_tool": True,
    },
}

SHARD_SPECS = {
    "real_zig_format": {
        "capability": "format",
        "doc": "Additive Zig format family aspect (zig fmt).",
        "shard": "zig",
        "suffix": "-zig",
    },
    "real_zig_lint": {
        "capability": "lint",
        "doc": "Additive Zig lint family aspect (zig ast-check).",
        "shard": "zig",
        "suffix": "-zig",
    },
    "real_zig_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Zig typecheck family aspect (zig -fno-emit-bin replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "zig",
        "suffix": "-zig",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = ["zig"],
    context_hooks = {"zig": compile_check_hook("zig")},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_zig_format_aspect = real_make_aspect(_CFG, "real_zig_format")

real_zig_lint_aspect = real_make_aspect(_CFG, "real_zig_lint")

real_zig_typecheck_aspect = real_make_aspect(_CFG, "real_zig_typecheck")

ZigShardProbeInfo = provider(
    doc = "Shard names the zig shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_zig_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "zig", ZigShardProbeInfo)

real_zig_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "zig", ZigShardProbeInfo)

real_zig_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "zig", ZigShardProbeInfo)
