"""Shard probes over every file in the core family; refresh the GENERATED blocks with `bazel run //core/modules:update_lang_projections`."""

# BEGIN GENERATED FAMILY TABLE LOADS
load("//core/quality:families/core.bzl", _CORE_HOOKS = "HOOKS", _CORE_HOOK_ORDER = "HOOK_ORDER", _CORE_SHARD_SPECS = "SHARD_SPECS", _CORE_TOOLS = "TOOLS")
load("//core/quality:families/cue.bzl", _CUE_HOOKS = "HOOKS", _CUE_HOOK_ORDER = "HOOK_ORDER", _CUE_SHARD_SPECS = "SHARD_SPECS", _CUE_TOOLS = "TOOLS")
load("//core/quality:families/html_template.bzl", _HTML_TEMPLATE_HOOKS = "HOOKS", _HTML_TEMPLATE_HOOK_ORDER = "HOOK_ORDER", _HTML_TEMPLATE_SHARD_SPECS = "SHARD_SPECS", _HTML_TEMPLATE_TOOLS = "TOOLS")
load("//core/quality:families/jsonnet.bzl", _JSONNET_HOOKS = "HOOKS", _JSONNET_HOOK_ORDER = "HOOK_ORDER", _JSONNET_SHARD_SPECS = "SHARD_SPECS", _JSONNET_TOOLS = "TOOLS")
load("//core/quality:families/latex.bzl", _LATEX_HOOKS = "HOOKS", _LATEX_HOOK_ORDER = "HOOK_ORDER", _LATEX_SHARD_SPECS = "SHARD_SPECS", _LATEX_TOOLS = "TOOLS")
load("//core/quality:families/nix.bzl", _NIX_HOOKS = "HOOKS", _NIX_HOOK_ORDER = "HOOK_ORDER", _NIX_SHARD_SPECS = "SHARD_SPECS", _NIX_TOOLS = "TOOLS")
load("//core/quality:families/oci_image.bzl", _OCI_IMAGE_HOOKS = "HOOKS", _OCI_IMAGE_HOOK_ORDER = "HOOK_ORDER", _OCI_IMAGE_SHARD_SPECS = "SHARD_SPECS", _OCI_IMAGE_TOOLS = "TOOLS")
load("//core/quality:families/pkl.bzl", _PKL_HOOKS = "HOOKS", _PKL_HOOK_ORDER = "HOOK_ORDER", _PKL_SHARD_SPECS = "SHARD_SPECS", _PKL_TOOLS = "TOOLS")
load("//core/quality:families/shell.bzl", _SHELL_HOOKS = "HOOKS", _SHELL_HOOK_ORDER = "HOOK_ORDER", _SHELL_SHARD_SPECS = "SHARD_SPECS", _SHELL_TOOLS = "TOOLS")
load("//core/quality:families/text.bzl", _TEXT_HOOKS = "HOOKS", _TEXT_HOOK_ORDER = "HOOK_ORDER", _TEXT_SHARD_SPECS = "SHARD_SPECS", _TEXT_TOOLS = "TOOLS")
load("//core/quality:families/typst.bzl", _TYPST_HOOKS = "HOOKS", _TYPST_HOOK_ORDER = "HOOK_ORDER", _TYPST_SHARD_SPECS = "SHARD_SPECS", _TYPST_TOOLS = "TOOLS")
load("//core/quality:families/xml.bzl", _XML_HOOKS = "HOOKS", _XML_HOOK_ORDER = "HOOK_ORDER", _XML_SHARD_SPECS = "SHARD_SPECS", _XML_TOOLS = "TOOLS")
load("//core/quality:families/yaml.bzl", _YAML_HOOKS = "HOOKS", _YAML_HOOK_ORDER = "HOOK_ORDER", _YAML_SHARD_SPECS = "SHARD_SPECS", _YAML_TOOLS = "TOOLS")
# END GENERATED FAMILY TABLE LOADS

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

# BEGIN GENERATED FAMILY TABLES
_FAMILY_TOOLS = _CORE_TOOLS | _CUE_TOOLS | _HTML_TEMPLATE_TOOLS | _JSONNET_TOOLS | _LATEX_TOOLS | _NIX_TOOLS | _OCI_IMAGE_TOOLS | _PKL_TOOLS | _SHELL_TOOLS | _TEXT_TOOLS | _TYPST_TOOLS | _XML_TOOLS | _YAML_TOOLS
_FAMILY_SHARD_SPECS = _CORE_SHARD_SPECS | _CUE_SHARD_SPECS | _HTML_TEMPLATE_SHARD_SPECS | _JSONNET_SHARD_SPECS | _LATEX_SHARD_SPECS | _NIX_SHARD_SPECS | _OCI_IMAGE_SHARD_SPECS | _PKL_SHARD_SPECS | _SHELL_SHARD_SPECS | _TEXT_SHARD_SPECS | _TYPST_SHARD_SPECS | _XML_SHARD_SPECS | _YAML_SHARD_SPECS
_FAMILY_HOOK_ORDER = _CORE_HOOK_ORDER + _CUE_HOOK_ORDER + _HTML_TEMPLATE_HOOK_ORDER + _JSONNET_HOOK_ORDER + _LATEX_HOOK_ORDER + _NIX_HOOK_ORDER + _OCI_IMAGE_HOOK_ORDER + _PKL_HOOK_ORDER + _SHELL_HOOK_ORDER + _TEXT_HOOK_ORDER + _TYPST_HOOK_ORDER + _XML_HOOK_ORDER + _YAML_HOOK_ORDER
_FAMILY_HOOKS = _CORE_HOOKS | _CUE_HOOKS | _HTML_TEMPLATE_HOOKS | _JSONNET_HOOKS | _LATEX_HOOKS | _NIX_HOOKS | _OCI_IMAGE_HOOKS | _PKL_HOOKS | _SHELL_HOOKS | _TEXT_HOOKS | _TYPST_HOOKS | _XML_HOOKS | _YAML_HOOKS
# END GENERATED FAMILY TABLES

_CFG = real_config(
    tools = _FAMILY_TOOLS,
    shard_specs = _FAMILY_SHARD_SPECS,
    hook_order = _FAMILY_HOOK_ORDER,
    context_hooks = _FAMILY_HOOKS,
)

QUALITY_CFG = _CFG

real_core_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "core", CoreShardProbeInfo)

real_core_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "core", CoreShardProbeInfo)

real_core_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "core", CoreShardProbeInfo)
