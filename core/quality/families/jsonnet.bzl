"""Jsonnet real quality aspects."""

load("//core/quality:family_hooks.bzl", "default_files_hook")
load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "jsonnetfmt": {
        "attr": attr.label(default = "@dx_tools//:jsonnetfmt", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "jsonnet",
    },
    "jsonnet_lint": {
        "attr": attr.label(default = "@dx_tools//:jsonnet_lint", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "jsonnet",
    },
}

SHARD_SPECS = {
    "real_jsonnet_format": {
        "capability": "format",
        "doc": "Additive Jsonnet format family aspect (jsonnetfmt).",
        "shard": "jsonnet",
        "suffix": "-jsonnet",
    },
    "real_jsonnet_lint": {
        "capability": "lint",
        "doc": "Additive Jsonnet lint family aspect (jsonnet-lint).",
        "shard": "jsonnet",
        "suffix": "-jsonnet",
    },
}

HOOK_ORDER = ["jsonnetfmt", "jsonnet_lint"]

HOOKS = {
    "jsonnetfmt": default_files_hook(["jsonnet", "libsonnet", "json"]),
    "jsonnet_lint": default_files_hook(["jsonnet", "libsonnet", "json"]),
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

real_jsonnet_format_aspect = real_make_aspect(_CFG, "real_jsonnet_format")

real_jsonnet_lint_aspect = real_make_aspect(_CFG, "real_jsonnet_lint")
