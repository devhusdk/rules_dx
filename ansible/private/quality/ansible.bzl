"""Ansible real quality aspects (ansible-lint, yamlfmt)."""

load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "ansible_lint": {
        "attr": attr.label(default = "@dx_prebuilt//:python_tool_ansible_lint_tool", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "heavy",
        "runfiles_env": True,
        "shard": "ansible",
    },
    "yamlfmt": {
        "attr": attr.label(default = "@dx_tools//:yamlfmt", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "ansible",
    },
}

SHARD_SPECS = {
    "real_ansible_format": {
        "capability": "format",
        "doc": "Additive Ansible format family aspect (yamlfmt).",
        "shard": "ansible",
    },
    "real_ansible_lint": {
        "capability": "lint",
        "doc": "Additive Ansible lint family aspect (ansible-lint).",
        "shard": "ansible",
    },
}

def _yamlfmt_hook(_target, _ctx, _resolved, _capability, _what, _shared):
    return real_context(options = {"args": ["document_start"]})

HOOK_ORDER = ["yamlfmt"]

HOOKS = {"yamlfmt": _yamlfmt_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
)

real_ansible_format_aspect = real_make_aspect(_CFG, "real_ansible_format")

real_ansible_lint_aspect = real_make_aspect(_CFG, "real_ansible_lint")

AnsibleShardProbeInfo = provider(
    doc = "Shard names the ansible shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_ansible_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "ansible", AnsibleShardProbeInfo)

real_ansible_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "ansible", AnsibleShardProbeInfo)
