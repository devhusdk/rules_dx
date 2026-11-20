"""Rego real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//rego/private:providers.bzl", "RegoInfo")

TOOLS = {
    "opa": {
        "attr": attr.label(default = "//config:tool_opa", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "rego",
    },
    "regal": {
        "attr": attr.label(default = "//config:tool_regal", allow_single_file = True, cfg = "exec"),
        "env": [["REGAL_DISABLE_VERSION_CHECK", "1"]],
        "kind": "file",
        "resources": "light",
        "shard": "rego",
    },
}

SHARD_SPECS = {
    "real_rego_format": {
        "capability": "format",
        "doc": "Additive Rego format family aspect (opa fmt).",
        "shard": "rego",
    },
    "real_rego_lint": {
        "capability": "lint",
        "doc": "Additive Rego lint family aspect (regal lint).",
        "shard": "rego",
    },
    "real_rego_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Rego typecheck family aspect (opa check).",
        "shard": "rego",
    },
}

def _opa_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if RegoInfo not in target:
        return real_context()
    return real_context(resolve_lists = [(target[RegoInfo].transitive_srcs, ["rego"])])

HOOK_ORDER = ["opa"]

HOOKS = {
    "opa": _opa_hook,
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
)

real_rego_format_aspect = real_make_aspect(_CFG, "real_rego_format")

real_rego_lint_aspect = real_make_aspect(_CFG, "real_rego_lint")

real_rego_typecheck_aspect = real_make_aspect(_CFG, "real_rego_typecheck")

RegoShardProbeInfo = provider(
    doc = "Shard names the rego shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_rego_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "rego", RegoShardProbeInfo)

real_rego_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "rego", RegoShardProbeInfo)

real_rego_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "rego", RegoShardProbeInfo)
