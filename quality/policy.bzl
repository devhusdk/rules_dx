"""Workspace quality-policy providers (freeze)."""

CAPABILITIES = ["lint", "typecheck", "format", "audit"]

FamilyPolicyInfo = provider(
    doc = "Tool-ID selection per capability for one quality policy family.",
    fields = {
        "audit": "List[str]: selected audit tool IDs, in policy order.",
        "disabled": "List[str]: capabilities this family turns off on purpose.",
        "family_id": "Str: policy-family ID owning this selection.",
        "format": "List[str]: selected formatter IDs, in policy order.",
        "lint": "List[str]: selected lint tool IDs, in policy order.",
        "typecheck": "List[str]: selected typecheck tool IDs, in policy order.",
    },
)

QualityPolicyInfo = provider(
    doc = "Canonical aggregate workspace policy: family ID to section.",
    fields = {
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
    )]

quality_family = rule(
    implementation = _quality_family_impl,
    attrs = {
        "audit": attr.string_list(
            default = [],
        ),
        "disabled": attr.string_list(
            default = [],
        ),
        "family_id": attr.string(),
        "format": attr.string_list(
            default = [],
        ),
        "lint": attr.string_list(
            default = [],
        ),
        "typecheck": attr.string_list(
            default = [],
        ),
    },
)

def _workspace_policy_impl(ctx):
    families = {}
    for section in ctx.attr.families:
        info = section[FamilyPolicyInfo]
        if info.family_id in families:
            fail("workspace_policy: family '" + info.family_id +
                 "' is provided twice")
        families[info.family_id] = info
    return [QualityPolicyInfo(families = families)]

workspace_policy = rule(
    implementation = _workspace_policy_impl,
    attrs = {
        "families": attr.label_list(
            providers = [FamilyPolicyInfo],
            default = [],
        ),
    },
)

def family_section_error(family_id, capability, selection, disabled, wired_tools):
    """Returns the diagnostic for one family/capability section, or ""."""
    if len(selection) == 0:
        if capability in disabled:
            return ""
        return ("policy: family '" + family_id + "' leaves capability '" + capability +
                "' unconfigured; select tools or disable '" + capability + "'")
    for tool in selection:
        if tool not in wired_tools:
            return ("policy: family '" + family_id + "' selects " + capability + " tool '" +
                    tool + "' with no wired executable; select one of " +
                    ", ".join(wired_tools) + " or disable '" + capability + "'")
    return ""
