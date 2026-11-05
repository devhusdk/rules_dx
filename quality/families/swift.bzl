"""Swift real quality aspects."""

load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

INDEX_STORE_REL = "periphery.indexstore"

TOOLS = {
    "periphery": {
        "attr": attr.label(default = "//config:tool_periphery", allow_files = True, cfg = "exec"),
        "fallback": " to a periphery binary",
        "kind": "file",
        "resources": "medium",
        "shard": "swift",
    },
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
}

SHARD_SPECS = {
    "real_swift_format": {
        "capability": "format",
        "doc": "Additive Swift format family aspect (swift-format).",
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

def _periphery_hook(target, ctx, _resolved, _capability, what, _shared):
    stores = []
    if OutputGroupInfo in target and "swift_index_store" in target[OutputGroupInfo]:
        stores = target[OutputGroupInfo]["swift_index_store"].to_list()
    if len(stores) != 1:
        fail(what + ": periphery needs the index store of a Swift target; build it with the swift.index_while_building feature")
    runtime = ctx.attr._periphery[DefaultInfo].default_runfiles.files.to_list()
    return real_context(tool_files = [(INDEX_STORE_REL, stores[0])], inputs = [stores[0]] + runtime)

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = ["periphery"],
    context_hooks = {"periphery": _periphery_hook},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_swift_format_aspect = real_make_aspect(_CFG, "real_swift_format")

real_swift_lint_aspect = real_make_aspect(_CFG, "real_swift_lint")
