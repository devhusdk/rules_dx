"""Workspace quality-policy providers (freeze)."""

load(":native_config.bzl", "DxNativeConfigInfo")

visibility(["//core/quality/..."])

CAPABILITIES = ["lint", "typecheck", "format", "audit"]

FamilyPolicyInfo = provider(
    doc = "Tool-ID selection per capability for one quality policy family.",
    fields = {
        "audit": "List[str]: selected audit tool IDs, in policy order.",
        "disabled": "List[str]: capabilities this family turns off on purpose.",
        "family_id": "Str: policy-family ID owning this selection.",
        "format": "List[str]: selected formatter IDs, in policy order.",
        "lint": "List[str]: selected lint tool IDs, in policy order.",
        "tier": "Str: tier (standard or strict) the family's tools run on, empty for the workspace default.",
        "typecheck": "List[str]: selected typecheck tool IDs, in policy order.",
    },
)

QualityPolicyInfo = provider(
    doc = "Canonical aggregate workspace policy: family ID to section.",
    fields = {
        "bases": "Dict[str, Target]: tool ID to the tier base config target, own bases over those of `extends`.",
        "configs": "Dict[str, Target]: tool ID to the native config target (carrying DxNativeConfigInfo) every target uses unless its aspect_hints bind one: `declared` over `bases`.",
        "declared": "Dict[str, Target]: tool ID to the config target declared in `configs`, own configs over those of `extends`.",
        "families": "Dict[str, FamilyPolicyInfo]: family ID to its section.",
    },
)

def _check_tool_ids(family_id, capability, tool_ids):
    seen = {}
    for tool_id in tool_ids:
        if type(tool_id) != "string" or tool_id == "":
            fail("quality_family (" + family_id + "): capability '" +
                 capability + "' holds a non-string or empty tool ID")
        if tool_id in seen:
            fail("quality_family (" + family_id + "): capability '" +
                 capability + "' selects tool '" + tool_id + "' twice")
        seen[tool_id] = True

def _check_disabled(family_id, disabled, selections):
    seen = {}
    for capability in disabled:
        if capability not in CAPABILITIES:
            fail("quality_family (" + family_id + "): disabled names unknown capability '" +
                 capability + "'; want one of " + ", ".join(CAPABILITIES))
        if capability in seen:
            fail("quality_family (" + family_id + "): capability '" + capability +
                 "' is disabled twice")
        if len(selections[capability]) > 0:
            fail("quality_family (" + family_id + "): capability '" + capability +
                 "' selects tools and is disabled; pick one")
        seen[capability] = True

def _quality_family_impl(ctx):
    family_id = ctx.attr.family_id
    if type(family_id) != "string" or family_id == "":
        fail("quality_family: family_id must be a non-empty string")
    selections = {capability: getattr(ctx.attr, capability) for capability in CAPABILITIES}
    for capability in CAPABILITIES:
        _check_tool_ids(family_id, capability, selections[capability])
    _check_disabled(family_id, ctx.attr.disabled, selections)
    return [FamilyPolicyInfo(
        family_id = family_id,
        lint = selections["lint"],
        typecheck = selections["typecheck"],
        format = selections["format"],
        audit = selections["audit"],
        disabled = ctx.attr.disabled,
        tier = ctx.attr.tier,
    )]

quality_family = rule(
    implementation = _quality_family_impl,
    doc = "Quality tool selection for one family.",
    attrs = {
        "audit": attr.string_list(
            doc = "Audit tool ids selected for this quality family.",
            default = [],
        ),
        "disabled": attr.string_list(
            doc = "Capabilities disabled for this family, which must select no tools.",
            default = [],
        ),
        "family_id": attr.string(doc = "Unique non-empty identifier for this quality family."),
        "format": attr.string_list(
            doc = "Format tool ids selected for this quality family.",
            default = [],
        ),
        "lint": attr.string_list(
            doc = "Lint tool ids selected for this quality family.",
            default = [],
        ),
        "tier": attr.string(
            doc = "Tier the family's tools run on; empty uses the workspace tier.",
        ),
        "typecheck": attr.string_list(
            doc = "Typecheck tool ids selected for this quality family.",
            default = [],
        ),
    },
)

def merge_policy(base, sections, disabled, has_base):
    """Merges family sections over a base policy."""
    if disabled and not has_base:
        return {}, "workspace_policy: 'disabled' needs 'extends'"
    seen = {}
    for family_id, _ in sections:
        if family_id in seen:
            return {}, "workspace_policy: family '" + family_id + "' is provided twice"
        seen[family_id] = True
    for family_id in disabled:
        if family_id not in base:
            return {}, "workspace_policy: cannot disable family '" + family_id + "', which the base policy does not have"
        if family_id in seen:
            return {}, "workspace_policy: family '" + family_id + "' is both disabled and provided"
    families = {k: v for k, v in base.items() if k not in disabled}
    for family_id, value in sections:
        families[family_id] = value
    return families, ""

def merge_configs(base, entries):
    """Merges native configs over a base policy's configs."""
    seen = {}
    for tool_id, _ in entries:
        if tool_id in seen:
            return {}, "workspace_policy: configs lists tool '" + tool_id + "' twice"
        seen[tool_id] = True
    configs = dict(base)
    for tool_id, value in entries:
        configs[tool_id] = value
    return configs, ""

def _workspace_policy_impl(ctx):
    base = {}
    base_bases = {}
    base_declared = {}
    if ctx.attr.extends:
        info = ctx.attr.extends[QualityPolicyInfo]
        base = info.families
        base_bases = info.bases
        base_declared = info.declared
    sections = []
    for section in ctx.attr.families:
        info = section[FamilyPolicyInfo]
        sections.append((info.family_id, info))
    families, error = merge_policy(base, sections, ctx.attr.disabled, bool(ctx.attr.extends))
    if error:
        fail(error)
    base_entries = [(config[DxNativeConfigInfo].tool_id, config) for config in ctx.attr.bases]
    bases, base_error = merge_configs(base_bases, base_entries)
    if base_error:
        fail(base_error)
    entries = [(config[DxNativeConfigInfo].tool_id, config) for config in ctx.attr.configs]
    declared, config_error = merge_configs(base_declared, entries)
    if config_error:
        fail(config_error)
    configs = dict(bases)
    configs.update(declared)
    return [QualityPolicyInfo(families = families, configs = configs, bases = bases, declared = declared)]

workspace_policy = rule(
    implementation = _workspace_policy_impl,
    doc = "Workspace quality policy built from family sections, optionally extending another policy.",
    attrs = {
        "bases": attr.label_list(
            doc = "Tier base config targets for tools with no bound file. Any config in `configs` or `extends` wins over a base.",
            providers = [DxNativeConfigInfo],
            default = [],
        ),
        "configs": attr.label_list(
            doc = "Native config targets (ruff_config, eslint_config, ...) bound for every target. A config in a target's aspect_hints wins; a config here wins over the one in `extends`.",
            providers = [DxNativeConfigInfo],
            default = [],
        ),
        "disabled": attr.string_list(
            doc = "Family IDs to drop from the policy named by `extends`.",
            default = [],
        ),
        "extends": attr.label(
            doc = "Base policy. Families here replace or add to the base families.",
            providers = [QualityPolicyInfo],
        ),
        "families": attr.label_list(
            doc = "Family policy sections combined into the workspace policy.",
            providers = [FamilyPolicyInfo],
            default = [],
        ),
    },
)

def family_section_error(family_id, capability, selection, disabled, wired_tools, unpinned = {}):
    """Returns the diagnostic for one family/capability section, or ""."""
    if len(selection) == 0:
        if capability in disabled:
            return ""
        return ("policy: family '" + family_id + "' leaves capability '" + capability +
                "' unconfigured; select tools or disable '" + capability + "'")
    for tool in selection:
        if tool in unpinned:
            return ("policy: family '" + family_id + "' selects " + capability + " tool '" +
                    tool + "', which has no pinned build on " + ", ".join(unpinned[tool]) +
                    "; disable '" + capability + "'")
        if tool not in wired_tools:
            return ("policy: family '" + family_id + "' selects " + capability + " tool '" +
                    tool + "' with no wired executable; select one of " +
                    ", ".join(wired_tools) + " or disable '" + capability + "'")
    return ""
