"""Dockerfile real quality aspects."""

load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "hadolint": {
        "attr": attr.label(default = "//config:tool_hadolint", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "dockerfile",
    },
}

SHARD_SPECS = {
    "real_dockerfile_lint": {
        "capability": "lint",
        "doc": "Additive Dockerfile lint family aspect (hadolint).",
        "shard": "dockerfile",
        "suffix": "-dockerfile",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = [],
    context_hooks = {},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_dockerfile_lint_aspect = real_make_aspect(_CFG, "real_dockerfile_lint")
