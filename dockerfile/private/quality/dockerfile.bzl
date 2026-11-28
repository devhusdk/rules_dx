"""Dockerfile real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_context", "real_family_tier", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "hadolint": {
        "attr": attr.label(default = "//config:tool_hadolint", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "dockerfile",
    },
    "trivy_config": {
        "attr": attr.label(default = "//config:tool_trivy", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "dockerfile",
    },
}

SHARD_SPECS = {
    "real_dockerfile_lint": {
        "capability": "lint",
        "doc": "Additive Dockerfile lint family aspect (hadolint).",
        "extra_attrs": {
            "_config_tier": attr.label(default = "//config:config_tier"),
        },
        "shard": "dockerfile",
    },
}

HOOK_ORDER = []

HOOKS = {}

def _hadolint_hook(_target, ctx, _resolved, _capability, _what, _shared):
    """Passes `strict` on the strict tier, which turns on the Dockerfile guard."""
    return real_context(options = {"args": ["strict"] if real_family_tier(ctx, "dockerfile") == "strict" else []})

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = ["hadolint"],
    context_hooks = {"hadolint": _hadolint_hook},
)

real_dockerfile_lint_aspect = real_make_aspect(_CFG, "real_dockerfile_lint")

DockerfileShardProbeInfo = provider(
    doc = "Shard names the dockerfile shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_dockerfile_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "dockerfile", DockerfileShardProbeInfo)
