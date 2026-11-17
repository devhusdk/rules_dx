"""Shard probes over every file in the core family."""

load("//core/quality:families/core.bzl", _CORE_HOOKS = "HOOKS", _CORE_HOOK_ORDER = "HOOK_ORDER", _CORE_SHARD_SPECS = "SHARD_SPECS", _CORE_TOOLS = "TOOLS")
load("//core/quality:families/cue.bzl", _CUE_HOOKS = "HOOKS", _CUE_HOOK_ORDER = "HOOK_ORDER", _CUE_SHARD_SPECS = "SHARD_SPECS", _CUE_TOOLS = "TOOLS")
load("//core/quality:families/pkl.bzl", _PKL_HOOKS = "HOOKS", _PKL_HOOK_ORDER = "HOOK_ORDER", _PKL_SHARD_SPECS = "SHARD_SPECS", _PKL_TOOLS = "TOOLS")
load("//core/quality:families/html_template.bzl", _HTML_TEMPLATE_SHARD_SPECS = "SHARD_SPECS", _HTML_TEMPLATE_TOOLS = "TOOLS")
load("//core/quality:families/jsonnet.bzl", _JSONNET_HOOKS = "HOOKS", _JSONNET_HOOK_ORDER = "HOOK_ORDER", _JSONNET_SHARD_SPECS = "SHARD_SPECS", _JSONNET_TOOLS = "TOOLS")
load("//core/quality:families/latex.bzl", _LATEX_SHARD_SPECS = "SHARD_SPECS", _LATEX_TOOLS = "TOOLS")
load("//core/quality:families/nix.bzl", _NIX_SHARD_SPECS = "SHARD_SPECS", _NIX_TOOLS = "TOOLS")
load("//core/quality:families/shell.bzl", _SHELL_SHARD_SPECS = "SHARD_SPECS", _SHELL_TOOLS = "TOOLS")
load("//core/quality:families/text.bzl", _TEXT_SHARD_SPECS = "SHARD_SPECS", _TEXT_TOOLS = "TOOLS")
load("//core/quality:families/typst.bzl", _TYPST_SHARD_SPECS = "SHARD_SPECS", _TYPST_TOOLS = "TOOLS")
load("//core/quality:families/xml.bzl", _XML_SHARD_SPECS = "SHARD_SPECS", _XML_TOOLS = "TOOLS")
load("//core/quality:families/yaml.bzl", _YAML_SHARD_SPECS = "SHARD_SPECS", _YAML_TOOLS = "TOOLS")
load("//core/quality:real_core.bzl", "real_config", "real_make_probe_aspect")

# Loaded from outside rules_dx (consumer fixtures, generated BUILD files or docs).
visibility("public")

CoreShardProbeInfo = provider(
    doc = "Shard names the core family shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

_CFG = real_config(
    tools = _CORE_TOOLS | _CUE_TOOLS | _HTML_TEMPLATE_TOOLS | _JSONNET_TOOLS | _LATEX_TOOLS | _NIX_TOOLS | _PKL_TOOLS | _SHELL_TOOLS | _TEXT_TOOLS | _TYPST_TOOLS | _XML_TOOLS | _YAML_TOOLS,
    shard_specs = _CORE_SHARD_SPECS | _CUE_SHARD_SPECS | _HTML_TEMPLATE_SHARD_SPECS | _JSONNET_SHARD_SPECS | _LATEX_SHARD_SPECS | _NIX_SHARD_SPECS | _PKL_SHARD_SPECS | _SHELL_SHARD_SPECS | _TEXT_SHARD_SPECS | _TYPST_SHARD_SPECS | _XML_SHARD_SPECS | _YAML_SHARD_SPECS,
    hook_order = _CORE_HOOK_ORDER + _CUE_HOOK_ORDER + _JSONNET_HOOK_ORDER + _PKL_HOOK_ORDER,
    context_hooks = _CORE_HOOKS | _CUE_HOOKS | _JSONNET_HOOKS | _PKL_HOOKS,
)

real_core_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "core", CoreShardProbeInfo)

real_core_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "core", CoreShardProbeInfo)

real_core_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "core", CoreShardProbeInfo)
