"""Finds tools that own the same concern for the same file class."""

load(":adapters.bzl", "REAL_CLASS_TO_FAMILY")
load(":tools.bzl", "TOOLS")

def _claims(tools):
    claims = {}
    for tool in tools:
        for capability, classes in tool.adapters.items():
            for class_id in classes:
                claims.setdefault("%s/%s" % (tool.concerns[capability], class_id), {})[tool.id] = capability
    return claims

def owners(tools = TOOLS):
    """Returns {"concern/class": sorted tool ids} for every concern and class claimed by more than one tool."""
    return {key: sorted(by_tool) for key, by_tool in _claims(tools).items() if len(by_tool) > 1}

def overlap_rows(tools = TOOLS):
    """Returns one (family, class, concern, first, second) row per overlapping tool pair, each tool with its capability."""
    rows = []
    claims = _claims(tools)
    for key, ids in owners(tools).items():
        concern, class_id = key.split("/", 1)
        for i, first in enumerate(ids):
            for second in ids[i + 1:]:
                rows.append((REAL_CLASS_TO_FAMILY.get(class_id, ""), class_id, concern, (first, claims[key][first]), (second, claims[key][second])))
    return sorted(rows)
