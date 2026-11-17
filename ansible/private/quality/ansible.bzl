"""Ansible real quality aspects (ansible-lint)."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "ansible_lint": {
        "attr": attr.label(default = "@dx_prebuilt//:python_tool_ansible_lint_tool", cfg = "exec", executable = True),
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
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_ansible_lint_aspect = real_make_aspect(_CFG, "real_ansible_lint")

AnsibleShardProbeInfo = provider(
    doc = "Shard names the ansible shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_ansible_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "ansible", AnsibleShardProbeInfo)
