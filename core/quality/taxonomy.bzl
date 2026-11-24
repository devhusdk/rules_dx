"""Quality taxonomy: every family with its wrapper owner and file classes."""

load("//core/modules:langs.bzl", "CORE_FAMILIES", "LANGS")

visibility("private")

# Kept apart from the label: the consumer surface walk reads a bare colon-defs string as a load from this package.
_PUBLIC_DEFS = "defs.bzl"

def _owner(name):
    return "other" if name == "other" else "//" + name + ":" + _PUBLIC_DEFS

CORE_TAXONOMY = {
    family: struct(owner = _owner(row["owner"]), classes = row["classes"])
    for family, row in CORE_FAMILIES.items()
}

LANG_JSON_FAMILIES = {
    row["quality"].get("family", lang): struct(
        owner = "//" + lang + ":" + _PUBLIC_DEFS,
        classes = row["quality"]["classes"],
    )
    for lang, row in LANGS.items()
    if "classes" in row.get("quality", {})
}

_ROWS = CORE_TAXONOMY | LANG_JSON_FAMILIES

TAXONOMY = {family: _ROWS[family] for family in sorted(_ROWS.keys())}

def taxonomy_schema_error(taxonomy = TAXONOMY):
    """Returns an error string for a malformed taxonomy, else the empty string."""
    seen = {}
    for family in sorted(taxonomy.keys()):
        entry = taxonomy[family]
        if not entry.owner:
            return "taxonomy: family '" + family + "' has an empty owner"
        if not entry.classes:
            return "taxonomy: family '" + family + "' has no classes"
        for class_id in entry.classes:
            if class_id in seen:
                return "taxonomy: class '" + class_id + "' listed under both '" + seen[class_id] + "' and '" + family + "'"
            seen[class_id] = family
    return ""

TAXONOMY_CLASSES = sorted([class_id for entry in TAXONOMY.values() for class_id in entry.classes])

CLASS_TO_FAMILY = {
    class_id: family
    for family, entry in TAXONOMY.items()
    for class_id in entry.classes
}

FAMILY_OWNERS = {family: entry.owner for family, entry in TAXONOMY.items()}
