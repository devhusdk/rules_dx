"""Go real quality aspects."""

load("@rules_go//go:def.bzl", "GoArchive")
load("//core/quality:family_hooks.bzl", "without")
load("//core/quality:go_analysis.bzl", "GO_TOOLCHAIN_TYPE", "GoAnalysisInfo", "go_analysis_aspect", "go_lang_version")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "errcheck": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "gofumpt": {
        "attr": attr.label(default = "//config:tool_gofumpt", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a gofumpt binary",
        "kind": "setting",
        "resources": "light",
        "shard": "go",
    },
    "gotype": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "govet": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "modfmt": {
        "attr": attr.label(default = "//config:tool_modfmt", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a modfmt binary",
        "kind": "setting",
        "resources": "light",
        "shard": "go",
    },
    "staticcheck": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
}

SHARD_SPECS = {
    "real_go_format": {
        "capability": "format",
        "doc": "Additive Go format family aspect (gofumpt).",
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_lint": {
        "capability": "lint",
        "doc": "Additive Go lint family aspect (delegated dx_govet, dx_errcheck and dx_staticcheck over rules_go packages).",
        "requires": [go_analysis_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Go typecheck family aspect (delegated dx_gotype over rules_go packages).",
        "requires": [go_analysis_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
}

def _gofumpt_lang(ctx, what):
    """Derives gofumpt -lang from the resolved rules_go SDK."""
    toolchain = ctx.toolchains[GO_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail(what + ": gofumpt needs a registered rules_go Go toolchain to derive -lang")
    return go_lang_version(toolchain.sdk.version)

def _go_analysis_hook(tool):
    def _hook(target, _ctx, resolved, _capability, what, _shared):
        if GoAnalysisInfo not in target:
            if GoArchive not in target:
                return without(resolved, tool)
            fail(what + ": " + tool + " needs the Go analysis aspect report for a rules_go target")
        report = target[GoAnalysisInfo].reports[tool]
        return real_context(upstream = [report], late_inputs = [report])

    return _hook

def _gofumpt_hook(_target, ctx, _resolved, _capability, what, _shared):
    return real_context(options = {"edition": _gofumpt_lang(ctx, what)})

HOOK_ORDER = ["errcheck", "gotype", "govet", "staticcheck", "gofumpt"]

HOOKS = {
    "gofumpt": _gofumpt_hook,
} | {tool: _go_analysis_hook(tool) for tool in ["errcheck", "gotype", "govet", "staticcheck"]}

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

real_go_format_aspect = real_make_aspect(_CFG, "real_go_format")

real_go_lint_aspect = real_make_aspect(_CFG, "real_go_lint")

real_go_typecheck_aspect = real_make_aspect(_CFG, "real_go_typecheck")

GoShardProbeInfo = provider(
    doc = "Shard names the go shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_go_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "go", GoShardProbeInfo)

real_go_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "go", GoShardProbeInfo)

real_go_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "go", GoShardProbeInfo)
