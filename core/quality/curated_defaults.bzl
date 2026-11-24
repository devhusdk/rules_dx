"""Curated quality defaults."""

load("//core/modules:langs.bzl", "CORE_FAMILIES", "LANGS")
load(":tool_defs.bzl", "is_canonical_token")

visibility("private")

CORE_DEFAULTS = {
    family: row["defaults"]
    for family, row in CORE_FAMILIES.items()
    if "defaults" in row
}

LANG_JSON_DEFAULTS = {
    row["quality"].get("family", lang): row["quality"]["defaults"]
    for lang, row in LANGS.items()
    if "defaults" in row.get("quality", {})
}

_ROWS = CORE_DEFAULTS | LANG_JSON_DEFAULTS

CURATED_DEFAULTS = {family: _ROWS[family] for family in sorted(_ROWS.keys())}

# Tools several families select; they always use the root profile.
CROSS_FAMILY_TOOLS = [
    "biome",
    "lychee",
    "markdown_check",
    "prettier",
    "trivy_config",
    "tsc",
    "typos",
    "yamlfmt",
]

def curated_schema_error():
    """Validates the curated-defaults schema."""
    for family in CURATED_DEFAULTS:
        if not is_canonical_token(family):
            return "curated defaults: non-canonical family '" + str(family) + "'"
        entry = CURATED_DEFAULTS[family]
        if sorted(entry.keys()) != ["audit", "format", "lint", "typecheck"]:
            return "curated defaults: family '" + family + "' must carry exactly audit/format/lint/typecheck"
        for capability in entry:
            for tool in entry[capability]:
                if not is_canonical_token(tool):
                    return "curated defaults: non-canonical tool '" + str(tool) + "' for family '" + family + "'"
    return ""
