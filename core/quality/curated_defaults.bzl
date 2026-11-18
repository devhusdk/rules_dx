"""Curated quality defaults."""

load("//core/modules:langs.bzl", "LANGS")
load(":tool_defs.bzl", "is_canonical_token")

visibility("private")

OTHER_DEFAULTS = {
    "core": {
        "audit": [],
        "format": ["yamlfmt"],
        "lint": ["actionlint", "zizmor"],
        "typecheck": [],
    },
    "cuda": {
        "audit": [],
        "format": ["clang_format"],
        "lint": ["clang_tidy", "typos"],
        "typecheck": [],
    },
    "go_module": {
        "audit": [],
        "format": ["modfmt"],
        "lint": ["typos"],
        "typecheck": [],
    },
    "graphql": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": [],
    },
    "html": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": [],
    },
    "json": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome"],
        "typecheck": [],
    },
    "markdown": {
        "audit": [],
        "format": [],
        "lint": ["lychee", "markdown_check", "typos"],
        "typecheck": [],
    },
    "nix": {
        "audit": [],
        "format": [],
        "lint": ["deadnix", "typos"],
        "typecheck": [],
    },
    "oci_image": {
        "audit": [],
        "format": [],
        "lint": ["trivy_image_config"],
        "typecheck": ["container_structure_test"],
    },
    "shell": {
        "audit": [],
        "format": ["shfmt"],
        "lint": ["shellcheck", "typos"],
        "typecheck": [],
    },
    "starlark": {
        "audit": [],
        "format": ["buildifier"],
        "lint": ["buildifier", "typos"],
        "typecheck": [],
    },
    "text": {
        "audit": [],
        "format": [],
        "lint": ["keep_sorted", "typos"],
        "typecheck": [],
    },
    "toml": {
        "audit": [],
        "format": ["taplo"],
        "lint": ["taplo", "typos"],
        "typecheck": [],
    },
    "yaml": {
        "audit": [],
        "format": ["yamlfmt"],
        "lint": ["yamllint"],
        "typecheck": [],
    },
}

LANG_JSON_DEFAULTS = {
    row["quality"].get("family", lang): row["quality"]["defaults"]
    for lang, row in LANGS.items()
    if "defaults" in row.get("quality", {})
}

_ROWS = OTHER_DEFAULTS | LANG_JSON_DEFAULTS

CURATED_DEFAULTS = {family: _ROWS[family] for family in sorted(_ROWS.keys())}

# Tools several families select; they always use the root profile.
CROSS_FAMILY_TOOLS = [
    "biome",
    "lychee",
    "markdown_check",
    "prettier",
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
