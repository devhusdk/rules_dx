"""Haskell real quality aspects."""

load("//quality:compile_check.bzl", "CompileCheckInfo", "compile_check_aspect", "compile_check_hook")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

HIE_REL = "weeder.hie"

TOOLS = {
    "ghc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "haskell",
    },
    "hlint": {
        "attr": attr.label(default = "//config:tool_hlint", allow_files = True, cfg = "exec"),
        "fallback": "=<label of an hlint executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "weeder": {
        "attr": attr.label(default = "//config:tool_weeder", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "ormolu": {
        "attr": attr.label(default = "//config:tool_ormolu", allow_files = True, cfg = "exec"),
        "fallback": "=<label of an ormolu executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
}

SHARD_SPECS = {
    "real_haskell_format": {
        "capability": "format",
        "doc": "Additive Haskell format family aspect (ormolu).",
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Haskell typecheck family aspect (ghc -fno-code replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_lint": {
        "capability": "lint",
        "doc": "Additive Haskell lint family aspect (hlint, weeder).",
        "requires": [compile_check_aspect],
        "shard": "haskell",
        "suffix": "-haskell",
    },
}

def _weeder_hook(target, ctx, resolved, _capability, _what, _shared):
    if CompileCheckInfo not in target or target[CompileCheckInfo].tool != "ghc":
        return without(resolved, "weeder")
    tool_files = []
    inputs = list(ctx.attr._weeder[DefaultInfo].default_runfiles.files.to_list())
    for i, hie in enumerate(target[CompileCheckInfo].scratch):
        stage(tool_files, inputs, "%s/%d" % (HIE_REL, i), hie)
    return real_context(tool_files = tool_files, inputs = inputs)

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = ["ghc", "weeder"],
    context_hooks = {"ghc": compile_check_hook("ghc"), "weeder": _weeder_hook},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_haskell_format_aspect = real_make_aspect(_CFG, "real_haskell_format")

real_haskell_lint_aspect = real_make_aspect(_CFG, "real_haskell_lint")

real_haskell_typecheck_aspect = real_make_aspect(_CFG, "real_haskell_typecheck")

HaskellShardProbeInfo = provider(
    doc = "Shard names the haskell shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_haskell_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "haskell", HaskellShardProbeInfo)

real_haskell_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "haskell", HaskellShardProbeInfo)

real_haskell_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "haskell", HaskellShardProbeInfo)
