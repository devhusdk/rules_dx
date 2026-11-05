"""Curated quality defaults."""


CURATED_DEFAULTS = {
    "astro": {
        "audit": [],
        "format": ["prettier"],
        "lint": [],
        "typecheck": ["tsc"],
    },
    "bicep": {
        "audit": [],
        "format": ["bicep"],
        "lint": ["bicep"],
        "typecheck": [],
    },
    "cc": {
        "audit": [],
        "format": ["clang_format"],
        "lint": [],
        "typecheck": [],
    },
    "cloudformation": {
        "audit": [],
        "format": [],
        "lint": ["cfn_lint"],
        "typecheck": [],
    },
    "dockerfile": {
        "audit": [],
        "format": [],
        "lint": ["hadolint"],
        "typecheck": [],
    },
    "go": {
        "audit": [],
        "format": ["gofumpt"],
        "lint": ["govet", "staticcheck"],
        "typecheck": ["gotype"],
    },
    "go_module": {
        "audit": [],
        "format": ["modfmt"],
        "lint": [],
        "typecheck": [],
    },
    "graphql": {
        "audit": [],
        "format": ["prettier"],
        "lint": [],
        "typecheck": [],
    },
    "html": {
        "audit": [],
        "format": ["prettier"],
        "lint": [],
        "typecheck": [],
    },
    "java": {
        "audit": [],
        "format": ["google_java_format"],
        "lint": ["checkstyle", "pmd", "spotbugs"],
        "typecheck": ["javac"],
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
        "lint": ["detekt", "ktlint"],
        "typecheck": ["kotlinc"],
    },
    "helm": {
        "audit": [],
        "format": [],
        "lint": ["helm"],
        "typecheck": [],
    },
    "kubernetes": {
        "audit": [],
        "format": [],
        "lint": ["kubeconform"],
        "typecheck": [],
    },
    "markdown": {
        "audit": [],
        "format": [],
        "lint": ["markdown_check", "vale"],
        "typecheck": [],
    },
    "mdx": {
        "audit": [],
        "format": ["remark"],
        "lint": [],
        "typecheck": ["tsc"],
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
    "scala": {
        "audit": [],
        "format": ["scalafmt"],
        "lint": [],
        "typecheck": ["scalac"],
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
    "svelte": {
        "audit": [],
        "format": ["prettier"],
        "lint": [],
        "typecheck": ["tsc"],
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
        "lint": ["taplo"],
        "typecheck": [],
    },
    "typescript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome"],
        "typecheck": ["tsc"],
    },
    "vue": {
        "audit": [],
        "format": ["prettier"],
        "lint": [],
        "typecheck": ["tsc"],
    },
}

def curated_families():
    """Returns the sorted curated families in the manifest."""
    return sorted(CURATED_DEFAULTS.keys())

def _is_canonical_token(text):
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def curated_schema_error():
    """Validates the curated-defaults schema."""
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
    return ""
