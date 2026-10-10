"""Frozen v1 curated quality defaults."""

CURATED_SCHEMA_VERSION = 1

CURATED_DEFAULTS = {
    "java": {
        "audit": [],
        "format": ["google_java_format"],
        "lint": ["checkstyle", "pmd", "spotbugs"],
        "typecheck": [],
    },
    "javascript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome"],
        "typecheck": [],
    },
    "json": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["biome"],
        "typecheck": [],
    },
    "kotlin": {
        "audit": [],
        "format": ["ktfmt"],
        "lint": ["ktlint"],
        "typecheck": [],
    },
    "markdown": {
        "audit": [],
        "format": [],
        "lint": ["markdown_check", "vale"],
        "typecheck": [],
    },
    "python": {
        "audit": [],
        "format": ["ruff"],
        "lint": ["pydoclint", "ruff"],
        "typecheck": ["ty"],
    },
    "rust": {
        "audit": [],
        "format": ["rustfmt"],
        "lint": ["clippy"],
        "typecheck": ["rustc"],
    },
    "shell": {
        "audit": [],
        "format": ["shfmt"],
        "lint": ["shellcheck"],
        "typecheck": [],
    },
    "starlark": {
        "audit": [],
        "format": ["buildifier"],
        "lint": ["buildifier"],
        "typecheck": [],
    },
    "text": {
        "audit": [],
        "format": [],
        "lint": ["keep_sorted"],
        "typecheck": [],
    },
    "toml": {
        "audit": [],
        "format": ["taplo"],
        "lint": ["taplo"],
        "typecheck": [],
    },
    "typescript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome"],
        "typecheck": ["tsc"],
    },
}

FORMAT_FROZEN = {
    "java": ["google_java_format"],
    "javascript": ["biome"],
    "json": ["prettier"],
    "kotlin": ["ktfmt"],
    "markdown": [],
    "python": ["ruff"],
    "rust": ["rustfmt"],
    "shell": ["shfmt"],
    "starlark": ["buildifier"],
    "text": [],
    "toml": ["taplo"],
    "typescript": ["biome"],
}

def curated_families():
    """Returns the sorted curated families in the versioned manifest."""
    return sorted(CURATED_DEFAULTS.keys())

def _is_canonical_token(text):
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def curated_schema_error():
    """Validates the versioned curated-defaults schema."""
    if CURATED_SCHEMA_VERSION != 1:
        return "curated defaults: unsupported schema v" + str(CURATED_SCHEMA_VERSION) + " (want v1)"
    for family in CURATED_DEFAULTS:
        if not _is_canonical_token(family):
            return "curated defaults: non-canonical family '" + str(family) + "'"
        entry = CURATED_DEFAULTS[family]
        if sorted(entry.keys()) != ["audit", "format", "lint", "typecheck"]:
            return "curated defaults: family '" + family + "' must carry exactly audit/format/lint/typecheck"
        for capability in entry:
            for tool in entry[capability]:
                if not _is_canonical_token(tool):
                    return "curated defaults: non-canonical tool '" + str(tool) + "' for family '" + family + "'"
        if family in FORMAT_FROZEN:
            if FORMAT_FROZEN[family] != entry["format"]:
                return "curated defaults: FORMAT_FROZEN drift for family '" + family + "' (formatter-set changes need a major release)"
        else:
            return "curated defaults: family '" + family + "' missing from FORMAT_FROZEN"
    for family in FORMAT_FROZEN:
        if family not in CURATED_DEFAULTS:
            return "curated defaults: FORMAT_FROZEN family '" + family + "' has no curated entry"
    return ""
