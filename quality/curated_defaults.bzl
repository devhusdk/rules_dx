"""Curated quality defaults."""


CURATED_DEFAULTS = {
    "astro": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": ["tsc"],
    },
    "bicep": {
        "audit": [],
        "format": ["bicep"],
        "lint": ["bicep", "typos"],
        "typecheck": [],
    },
    "cc": {
        "audit": [],
        "format": ["clang_format"],
        "lint": ["typos"],
        "typecheck": [],
    },
    "cloudformation": {
        "audit": [],
        "format": [],
        "lint": ["cfn_lint", "typos"],
        "typecheck": [],
    },
    "dockerfile": {
        "audit": [],
        "format": [],
        "lint": ["hadolint", "typos"],
        "typecheck": [],
    },
    "go": {
        "audit": [],
        "format": ["gofumpt"],
        "lint": ["govet", "staticcheck", "typos"],
        "typecheck": ["gotype"],
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
    "java": {
        "audit": [],
        "format": ["google_java_format"],
        "lint": ["checkstyle", "pmd", "spotbugs", "typos"],
        "typecheck": ["javac"],
    },
    "javascript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome", "typos"],
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
        "lint": ["detekt", "ktlint", "typos"],
        "typecheck": ["kotlinc"],
    },
    "helm": {
        "audit": [],
        "format": [],
        "lint": ["helm", "typos"],
        "typecheck": [],
    },
    "kubernetes": {
        "audit": [],
        "format": [],
        "lint": ["kubeconform", "typos"],
        "typecheck": [],
    },
    "markdown": {
        "audit": [],
        "format": [],
        "lint": ["lychee", "markdown_check", "typos", "vale"],
        "typecheck": [],
    },
    "mdx": {
        "audit": [],
        "format": ["remark"],
        "lint": ["typos"],
        "typecheck": ["tsc"],
    },
    "python": {
        "audit": [],
        "format": ["ruff"],
        "lint": ["pydoclint", "ruff", "typos", "vulture"],
        "typecheck": ["ty"],
    },
    "rust": {
        "audit": [],
        "format": ["rustfmt"],
        "lint": ["clippy", "typos"],
        "typecheck": ["rustc"],
    },
    "scala": {
        "audit": [],
        "format": ["scalafmt"],
        "lint": ["typos"],
        "typecheck": ["scalac"],
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
    "svelte": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
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
        "lint": ["taplo", "typos"],
        "typecheck": [],
    },
    "typescript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome", "typos"],
        "typecheck": ["tsc"],
    },
    "vue": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
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
