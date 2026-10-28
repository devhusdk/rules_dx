"""Writes the support matrix facts JSON from the language list and the quality registry."""

load("//quality:adapters.bzl", "NOT_APPLICABLE", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")
load("//quality:parity_tests.bzl", "PARITY_DEFERRED")
load("//quality:support.bzl", "SUPPORT_RECORDS")
load(":languages.bzl", "COMPUTED_CONSTRAINTS", "GAP_ENFORCEMENT", "GAP_ISSUES", "NOT_LANGUAGES", "PLATFORMS", "ROWS", "SECTIONS", "SUPPORT_MATRIX_SCHEMA_VERSION")

_CAPABILITIES = ["lint", "format", "typecheck"]

_ROW_KEYS = ["ci_tests", "consumer_tests", "families", "name", "section", "title", "workflows"]

_BUILD_KEYS = ["ci_tests", "consumer_tests", "workflows"]

def _quality():
    families = {}
    for family in REAL_CLASS_TO_FAMILY.values():
        families[family] = {capability: {} for capability in _CAPABILITIES}
    for tool, capabilities in REAL_ADAPTERS.items():
        for capability, classes in capabilities.items():
            if capability not in _CAPABILITIES:
                continue
            for class_id in classes:
                families[REAL_CLASS_TO_FAMILY[class_id]][capability][tool] = True
    return {
        family: {capability: sorted(tools[capability].keys()) for capability in _CAPABILITIES}
        for family, tools in families.items()
    }

def support_matrix_facts_dict():
    """Returns the validated facts as a dict."""
    if SUPPORT_MATRIX_SCHEMA_VERSION != 1:
        fail("support matrix: unsupported schema v" + str(SUPPORT_MATRIX_SCHEMA_VERSION))
    quality = _quality()
    names = {}
    for row in ROWS:
        for key in row:
            if key not in _ROW_KEYS:
                fail("support matrix: row " + row["name"] + " has unknown key " + key)
        if row["name"] in names:
            fail("support matrix: row " + row["name"] + " is listed twice")
        names[row["name"]] = True
        if row["name"] in NOT_LANGUAGES:
            fail("support matrix: " + row["name"] + " is both a row and not a language")
        if row["section"] not in SECTIONS:
            fail("support matrix: row " + row["name"] + " has unknown section " + row["section"])
        for family in row.get("families", [row["name"]]):
            if family not in quality:
                fail("support matrix: row " + row["name"] + " names unknown quality family " + family)
        if row["section"] == "formats":
            for key in _BUILD_KEYS:
                if row.get(key, []):
                    fail("support matrix: format row " + row["name"] + " sets " + key)
    support = []
    for record in SUPPORT_RECORDS:
        support.append({
            "capability": record["capability"],
            "family": record["family"],
            "platforms": record["platforms"],
            "tool": record["tool"],
        })
    deferred = {}
    for class_id in PARITY_DEFERRED:
        deferred[REAL_CLASS_TO_FAMILY.get(class_id, class_id)] = True
    return {
        "computed_constraints": COMPUTED_CONSTRAINTS,
        "deferred_families": sorted(deferred.keys()),
        "gap_enforcement": GAP_ENFORCEMENT,
        "gap_issues": GAP_ISSUES,
        "not_applicable": NOT_APPLICABLE,
        "not_languages": sorted(NOT_LANGUAGES),
        "platforms": PLATFORMS,
        "quality": quality,
        "rows": [
            {
                "ci_tests": row.get("ci_tests", []),
                "consumer_tests": row.get("consumer_tests", []),
                "families": row.get("families", [row["name"]]),
                "name": row["name"],
                "section": row["section"],
                "title": row["title"],
                "workflows": row.get("workflows", []),
            }
            for row in sorted(ROWS, key = lambda row: row["name"])
        ],
        "schema_version": SUPPORT_MATRIX_SCHEMA_VERSION,
        "sections": SECTIONS,
        "support": sorted(support, key = lambda record: (record["family"], record["capability"], record["tool"])),
    }

def _support_matrix_facts_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent(support_matrix_facts_dict(), indent = "  ") + "\n")
    return [DefaultInfo(files = depset([out]))]

support_matrix_facts = rule(
    implementation = _support_matrix_facts_impl,
    doc = "Writes the support matrix facts JSON.",
)
