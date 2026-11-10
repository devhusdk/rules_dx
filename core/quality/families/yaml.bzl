"""Yaml real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "yamlfmt": {
        "attr": attr.label(default = "@dx_tools//:yamlfmt", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "yaml",
    },
    "yamllint": {
        "attr": attr.label(default = "//core/quality/tools/python:yamllint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "yaml",
    },
}

SHARD_SPECS = {
    "real_yaml_format": {
        "capability": "format",
        "doc": "Additive YAML format family aspect (yamlfmt).",
        "shard": "yaml",
        "suffix": "-yaml",
    },
    "real_yaml_lint": {
        "capability": "lint",
        "doc": "Additive YAML lint family aspect (yamllint).",
        "shard": "yaml",
        "suffix": "-yaml",
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

real_yaml_format_aspect = real_make_aspect(_CFG, "real_yaml_format")

real_yaml_lint_aspect = real_make_aspect(_CFG, "real_yaml_lint")
