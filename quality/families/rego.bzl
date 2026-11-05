"""Rego real quality aspects."""

load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//rego/rules:providers.bzl", "RegoInfo")

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
        "suffix": "-rego",
    },
    "real_rego_lint": {
        "capability": "lint",
        "doc": "Additive Rego lint family aspect (regal lint).",
        "shard": "rego",
        "suffix": "-rego",
    },
    "real_rego_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Rego typecheck family aspect (opa check).",
        "shard": "rego",
        "suffix": "-rego",
    },
}

def _opa_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if RegoInfo not in target:
        return real_context()
    pairs = []
    for f in target[RegoInfo].transitive_srcs.to_list():
        ws_path = f.short_path
        if ".." in ws_path.split("/") or f.extension != "rego":
            continue
        pairs.append((ws_path, f, False))
    return real_context(resolve_pairs = pairs)


HOOK_ORDER = ["opa"]

HOOKS = {
    "opa": _opa_hook,
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

real_rego_format_aspect = real_make_aspect(_CFG, "real_rego_format")

real_rego_lint_aspect = real_make_aspect(_CFG, "real_rego_lint")

real_rego_typecheck_aspect = real_make_aspect(_CFG, "real_rego_typecheck")
