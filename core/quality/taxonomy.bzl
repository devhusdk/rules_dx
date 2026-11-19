"""Quality taxonomy: every family with its wrapper owner and file classes."""

load("//core/modules:langs.bzl", "LANGS")

visibility("private")

OTHER_FAMILIES = {
    "core": struct(
        owner = "other",
        classes = ["github_workflow"],
    ),
    "csharp": struct(
        owner = "@rules_dx//csharp:defs.bzl",
        classes = ["csharp"],
    ),
    "css": struct(
        owner = "//javascript:defs.bzl",
        classes = ["css", "less", "scss"],
    ),
    "cuda": struct(
        owner = "//cuda:defs.bzl",
        classes = ["cuda"],
    ),
    "cue": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["cue"],
    ),
    "fsharp": struct(
        owner = "@rules_dx//fsharp:defs.bzl",
        classes = ["fsharp"],
    ),
    "gherkin": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["gherkin"],
    ),
    "go_module": struct(
        owner = "other",
        classes = ["go_module"],
    ),
    "graphql": struct(
        owner = "//javascript:defs.bzl",
        classes = ["graphql"],
    ),
    "html": struct(
        owner = "//javascript:defs.bzl",
        classes = ["html"],
    ),
    "html_template": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["html_template"],
    ),
    "json": struct(
        owner = "other",
        classes = ["json", "json5", "jsonc"],
    ),
    "jsonnet": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["jsonnet"],
    ),
    "jupyter": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["jupyter"],
    ),
    "latex": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["latex"],
    ),
    "markdown": struct(
        owner = "other",
        classes = ["markdown"],
    ),
    "nix": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["nix"],
    ),
    "objc": struct(
        owner = "//cc:defs.bzl",
        classes = ["objc", "objcpp"],
    ),
    "oci_image": struct(
        owner = "//deploy:defs.bzl",
        classes = ["oci_image"],
    ),
    "pkl": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["pkl"],
    ),
    "protobuf": struct(
        owner = "other",
        classes = ["protobuf"],
    ),
    "rst": struct(
        owner = "//rst:defs.bzl",
        classes = ["rst"],
    ),
    "shell": struct(
        owner = "other",
        classes = ["shell"],
    ),
    "starlark": struct(
        owner = "other",
        classes = ["starlark"],
    ),
    "text": struct(
        owner = "other",
        classes = ["text"],
    ),
    "toml": struct(
        owner = "other",
        classes = ["toml"],
    ),
    "typst": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["typst"],
    ),
    "xml": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["xml"],
    ),
    "yaml": struct(
        owner = "//core/quality:defs.bzl",
        classes = ["yaml"],
    ),
}

LANG_JSON_FAMILIES = {
    row["quality"].get("family", lang): struct(
        owner = "//" + lang + ":defs.bzl",
        classes = row["quality"]["classes"],
    )
    for lang, row in LANGS.items()
    if "classes" in row.get("quality", {})
}

_ROWS = OTHER_FAMILIES | LANG_JSON_FAMILIES

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
