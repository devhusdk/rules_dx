"""CloudFormation real quality aspects (cfn-lint)."""

load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "cfn_lint": {
        "attr": attr.label(default = "//quality/tools/python:cfn_lint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "cloudformation",
    },
}

SHARD_SPECS = {
    "real_cloudformation_lint": {
        "capability": "lint",
        "doc": "Additive CloudFormation lint family aspect (cfn-lint).",
        "shard": "cloudformation",
        "suffix": "-cloudformation",
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

real_cloudformation_lint_aspect = real_make_aspect(_CFG, "real_cloudformation_lint")
