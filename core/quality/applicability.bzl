"""Quality applicability helpers (freeze)."""

load(":policy.bzl", "CAPABILITIES")

def capability_selection(tools_by_capability, capability):
    """Returns the policy-ordered tool-ID selection for one capability."""
    if capability not in CAPABILITIES:
        fail("applicability: unknown capability '" + capability +
             "': want one of " + ", ".join(CAPABILITIES))
    return tools_by_capability.get(capability, [])

def selected_adapters(selection, adapters):
    """Resolves a policy-ordered tool-ID selection against adapter metadata."""
    ordered = []
    seen = {}
    for tool_id in selection:
        if tool_id not in adapters:
            fail("applicability: unknown tool '" + tool_id +
                 "': not in the ruleset adapter registry")
        if tool_id not in seen:
            seen[tool_id] = True
            ordered.append(tool_id)
    return ordered

def effective_classes(target_classes, adapter_classes, policy_classes):
    """Returns the sorted three-way class intersection for one tool stage."""
    adapter_set = {c: True for c in adapter_classes}
    policy_set = {c: True for c in policy_classes}
    effective = {}
    for c in target_classes:
        if c in adapter_set and c in policy_set:
            effective[c] = True
    return sorted(effective.keys())
