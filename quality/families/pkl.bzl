"""Pkl real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//quality/tools/jvm:jar_lsp_tool.bzl", "PklLspToolInfo")

TOOLS = {
    "pkl": {
        "attr": attr.label(default = "//config:tool_pkl", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "medium",
        "shard": "pkl",
    },
    "pkl_lsp": {
        "attr": attr.label(default = "//quality/tools/jvm:pkl_lsp", cfg = "exec", executable = True),
        "extra": "runfiles",
        "kind": "executable",
        "resources": "heavy",
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
    "real_pkl_lint": {
        "capability": "lint",
        "doc": "Additive Pkl lint family aspect (pkl-lsp diagnostics through lsp_check).",
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

_FILES_HOOK = default_files_hook(["pkl"], except_capability = "format")

def _pkl_hook(target, ctx, resolved, capability, what, shared):
    context = _FILES_HOOK(target, ctx, resolved, capability, what, shared)
    if capability != "lint":
        return context
    bundle = ctx.attr._pkl_lsp[PklLspToolInfo]
    return real_context(
        resolve_pairs = context.resolve_pairs,
        options = {"path_tools": [("java", bundle.java), ("pkl-lsp.jar", bundle.jar)]},
    )

HOOK_ORDER = ["pkl"]

HOOKS = {"pkl": _pkl_hook}

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

real_pkl_lint_aspect = real_make_aspect(_CFG, "real_pkl_lint")

PklShardProbeInfo = provider(
    doc = "Shard names the pkl shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_pkl_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "pkl", PklShardProbeInfo)

real_pkl_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "pkl", PklShardProbeInfo)

real_pkl_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "pkl", PklShardProbeInfo)
