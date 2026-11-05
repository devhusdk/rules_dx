"""Ansible real quality aspects (ansible-lint)."""

load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "ansible_lint": {
        "attr": attr.label(default = "//quality/tools/python:ansible_lint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "heavy",
        "runfiles_env": True,
        "shard": "ansible",
    },
}

SHARD_SPECS = {
    "real_ansible_lint": {
        "capability": "lint",
        "doc": "Additive Ansible lint family aspect (ansible-lint).",
        "shard": "ansible",
        "suffix": "-ansible",
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

real_ansible_lint_aspect = real_make_aspect(_CFG, "real_ansible_lint")
