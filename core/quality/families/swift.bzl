"""Swift real quality aspects."""

load("//core/quality:compile_check.bzl", "compile_check_aspect", "compile_check_hook")
load("//core/quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "swift_format": {
        "attr": attr.label(default = "//config:tool_swift_format", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "swift",
        "target_tool": True,
    },
    "swift_lint": {
        "attr": attr.label(default = "//config:tool_swift_lint", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "swift",
        "target_tool": True,
    },
    "swiftc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "swift",
    },
}

SHARD_SPECS = {
    "real_swift_format": {
        "capability": "format",
        "doc": "Additive Swift format family aspect (swift-format).",
        "shard": "swift",
        "suffix": "-swift",
    },
    "real_swift_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Swift typecheck family aspect (swiftc -typecheck replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "swift",
        "suffix": "-swift",
    },
    "real_swift_lint": {
        "capability": "lint",
        "doc": "Additive Swift lint family aspect (swift-lint).",
        "shard": "swift",
        "suffix": "-swift",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = ["swiftc"],
    context_hooks = {"swiftc": compile_check_hook("swiftc")},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_swift_format_aspect = real_make_aspect(_CFG, "real_swift_format")

real_swift_lint_aspect = real_make_aspect(_CFG, "real_swift_lint")

real_swift_typecheck_aspect = real_make_aspect(_CFG, "real_swift_typecheck")

SwiftShardProbeInfo = provider(
    doc = "Shard names the swift shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_swift_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "swift", SwiftShardProbeInfo)

real_swift_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "swift", SwiftShardProbeInfo)

real_swift_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "swift", SwiftShardProbeInfo)
