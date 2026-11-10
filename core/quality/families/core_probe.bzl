"""Shard probes over every file in the core family."""

load("//ansible/private/quality:ansible.bzl", _ANSIBLE_SHARD_SPECS = "SHARD_SPECS", _ANSIBLE_TOOLS = "TOOLS")
load("//bicep/private/quality:bicep.bzl", _BICEP_HOOKS = "HOOKS", _BICEP_HOOK_ORDER = "HOOK_ORDER", _BICEP_SHARD_SPECS = "SHARD_SPECS", _BICEP_TOOLS = "TOOLS")
load("//cloudformation/private/quality:cloudformation.bzl", _CLOUDFORMATION_SHARD_SPECS = "SHARD_SPECS", _CLOUDFORMATION_TOOLS = "TOOLS")
load("//core/quality:families/cmake.bzl", _CMAKE_SHARD_SPECS = "SHARD_SPECS", _CMAKE_TOOLS = "TOOLS")
load("//core/quality:families/core.bzl", _CORE_HOOKS = "HOOKS", _CORE_HOOK_ORDER = "HOOK_ORDER", _CORE_SHARD_SPECS = "SHARD_SPECS", _CORE_TOOLS = "TOOLS")
load("//core/quality:families/cue.bzl", _CUE_HOOKS = "HOOKS", _CUE_HOOK_ORDER = "HOOK_ORDER", _CUE_SHARD_SPECS = "SHARD_SPECS", _CUE_TOOLS = "TOOLS")
load("//dockerfile/private/quality:dockerfile.bzl", _DOCKERFILE_SHARD_SPECS = "SHARD_SPECS", _DOCKERFILE_TOOLS = "TOOLS")
load("//core/quality:families/html_template.bzl", _HTML_TEMPLATE_SHARD_SPECS = "SHARD_SPECS", _HTML_TEMPLATE_TOOLS = "TOOLS")
load("//core/quality:families/jsonnet.bzl", _JSONNET_HOOKS = "HOOKS", _JSONNET_HOOK_ORDER = "HOOK_ORDER", _JSONNET_SHARD_SPECS = "SHARD_SPECS", _JSONNET_TOOLS = "TOOLS")
load("//kubernetes/private/quality:kubernetes.bzl", _KUBERNETES_HOOKS = "HOOKS", _KUBERNETES_HOOK_ORDER = "HOOK_ORDER", _KUBERNETES_SHARD_SPECS = "SHARD_SPECS", _KUBERNETES_TOOLS = "TOOLS")
load("//core/quality:families/latex.bzl", _LATEX_SHARD_SPECS = "SHARD_SPECS", _LATEX_TOOLS = "TOOLS")
load("//core/quality:families/make.bzl", _MAKE_SHARD_SPECS = "SHARD_SPECS", _MAKE_TOOLS = "TOOLS")
load("//core/quality:families/nix.bzl", _NIX_SHARD_SPECS = "SHARD_SPECS", _NIX_TOOLS = "TOOLS")
load("//rego/private/quality:rego.bzl", _REGO_HOOKS = "HOOKS", _REGO_HOOK_ORDER = "HOOK_ORDER", _REGO_SHARD_SPECS = "SHARD_SPECS", _REGO_TOOLS = "TOOLS")
load("//core/quality:families/shell.bzl", _SHELL_SHARD_SPECS = "SHARD_SPECS", _SHELL_TOOLS = "TOOLS")
load("//core/quality:families/sql.bzl", _SQL_HOOKS = "HOOKS", _SQL_HOOK_ORDER = "HOOK_ORDER", _SQL_SHARD_SPECS = "SHARD_SPECS", _SQL_TOOLS = "TOOLS")
load("//terraform/private/quality:terraform.bzl", _TERRAFORM_HOOKS = "HOOKS", _TERRAFORM_HOOK_ORDER = "HOOK_ORDER", _TERRAFORM_SHARD_SPECS = "SHARD_SPECS", _TERRAFORM_TOOLS = "TOOLS")
load("//core/quality:families/text.bzl", _TEXT_SHARD_SPECS = "SHARD_SPECS", _TEXT_TOOLS = "TOOLS")
load("//core/quality:families/typst.bzl", _TYPST_SHARD_SPECS = "SHARD_SPECS", _TYPST_TOOLS = "TOOLS")
load("//core/quality:families/xml.bzl", _XML_SHARD_SPECS = "SHARD_SPECS", _XML_TOOLS = "TOOLS")
load("//core/quality:families/yaml.bzl", _YAML_SHARD_SPECS = "SHARD_SPECS", _YAML_TOOLS = "TOOLS")
load("//core/quality:real_core.bzl", "real_config", "real_make_probe_aspect")

CoreShardProbeInfo = provider(
    doc = "Shard names the core family shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

_CFG = real_config(
    tools = _ANSIBLE_TOOLS | _BICEP_TOOLS | _CLOUDFORMATION_TOOLS | _CMAKE_TOOLS | _CORE_TOOLS | _CUE_TOOLS | _DOCKERFILE_TOOLS | _HTML_TEMPLATE_TOOLS | _JSONNET_TOOLS | _KUBERNETES_TOOLS | _LATEX_TOOLS | _MAKE_TOOLS | _NIX_TOOLS | _REGO_TOOLS | _SHELL_TOOLS | _SQL_TOOLS | _TERRAFORM_TOOLS | _TEXT_TOOLS | _TYPST_TOOLS | _XML_TOOLS | _YAML_TOOLS,
    shard_specs = _ANSIBLE_SHARD_SPECS | _BICEP_SHARD_SPECS | _CLOUDFORMATION_SHARD_SPECS | _CMAKE_SHARD_SPECS | _CORE_SHARD_SPECS | _CUE_SHARD_SPECS | _DOCKERFILE_SHARD_SPECS | _HTML_TEMPLATE_SHARD_SPECS | _JSONNET_SHARD_SPECS | _KUBERNETES_SHARD_SPECS | _LATEX_SHARD_SPECS | _MAKE_SHARD_SPECS | _NIX_SHARD_SPECS | _REGO_SHARD_SPECS | _SHELL_SHARD_SPECS | _SQL_SHARD_SPECS | _TERRAFORM_SHARD_SPECS | _TEXT_SHARD_SPECS | _TYPST_SHARD_SPECS | _XML_SHARD_SPECS | _YAML_SHARD_SPECS,
    hook_order = _CORE_HOOK_ORDER + _CUE_HOOK_ORDER + _REGO_HOOK_ORDER + _BICEP_HOOK_ORDER + _TERRAFORM_HOOK_ORDER + _JSONNET_HOOK_ORDER + _KUBERNETES_HOOK_ORDER + _SQL_HOOK_ORDER,
    context_hooks = _BICEP_HOOKS | _CORE_HOOKS | _CUE_HOOKS | _JSONNET_HOOKS | _KUBERNETES_HOOKS | _REGO_HOOKS | _SQL_HOOKS | _TERRAFORM_HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_core_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "core", CoreShardProbeInfo)

real_core_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "core", CoreShardProbeInfo)

real_core_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "core", CoreShardProbeInfo)
