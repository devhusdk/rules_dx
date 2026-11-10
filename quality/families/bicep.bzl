"""Bicep real quality aspects."""

load("//bicep/private:providers.bzl", "BicepInfo")
load("//quality:family_hooks.bzl", "stage")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

TOOLS = {
    "bicep": {
        "attr": attr.label(default = "@dx_tools//:bicep", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "medium",
        "shard": "bicep",
    },
    "trivy_config": {
        "attr": attr.label(default = "//config:tool_trivy", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "bicep",
    },
}

SHARD_SPECS = {
    "real_bicep_format": {
        "capability": "format",
        "doc": "Additive Bicep format family aspect (bicep format).",
        "shard": "bicep",
        "suffix": "-bicep",
    },
    "real_bicep_lint": {
        "capability": "lint",
        "doc": "Additive Bicep lint family aspect (bicep lint).",
        "shard": "bicep",
        "suffix": "-bicep",
    },
}

def _bicep_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if BicepInfo not in target:
        return real_context()
    tool_files = []
    inputs = []
    for f in target[BicepInfo].transitive_registry.to_list():
        rel = f.short_path.partition("/home/.bicep/")[2]
        if rel:
            stage(tool_files, inputs, "dx-bicep/home/.bicep/" + rel, f)
    pairs = []
    for f in depset(transitive = [target[BicepInfo].transitive_srcs, target[BicepInfo].transitive_data]).to_list():
        ws_path = f.short_path
        if ".." in ws_path.split("/"):
            continue
        pairs.append((ws_path, f, False))
    return real_context(tool_files = tool_files, inputs = inputs, resolve_pairs = pairs)

def _trivy_hook(target, ctx, resolved, capability, what, shared):
    context = _bicep_hook(target, ctx, resolved, capability, what, shared)
    if BicepInfo not in target:
        return context
    return real_context(
        tool_files = context.tool_files,
        inputs = context.inputs + [ctx.file._bicep],
        resolve_pairs = context.resolve_pairs,
        options = {"path_tools": [("bicep", ctx.file._bicep)]},
    )

HOOK_ORDER = ["bicep", "trivy_config"]

HOOKS = {
    "bicep": _bicep_hook,
    "trivy_config": _trivy_hook,
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

real_bicep_format_aspect = real_make_aspect(_CFG, "real_bicep_format")

real_bicep_lint_aspect = real_make_aspect(_CFG, "real_bicep_lint")
