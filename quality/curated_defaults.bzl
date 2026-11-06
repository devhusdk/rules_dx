"""Curated quality defaults."""


# periphery stays opt-in: macOS arm64 only, needs an index-store build.
# Julia lints with julialint only. Its extra fetch over typos is the dx_julia_lint package set
# (21 packages, about 4.2 MB); the 1.12.5 runtime (about 288 MB on linux_x86_64) is already
# fetched to precompile any julia_library (#8048). JET and JuliaFormatter stay opt-in.
CURATED_DEFAULTS = {
    "astro": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": ["tsc"],
        "unused": [],
    },
    "bicep": {
        "audit": [],
        "format": ["bicep"],
        "lint": ["bicep", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "cc": {
        "audit": [],
        "format": ["clang_format"],
        "lint": ["typos"],
        "typecheck": [],
        "unused": ["cppcheck"],
    },
    "cloudformation": {
        "audit": [],
        "format": [],
        "lint": ["cfn_lint", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "d": {
        "audit": [],
        "format": ["dfmt"],
        "lint": ["dscanner", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "dart": {
        "audit": [],
        "format": ["dart"],
        "lint": ["dart", "typos"],
        "typecheck": ["dart"],
        "unused": [],
    },
    "dockerfile": {
        "audit": [],
        "format": [],
        "lint": ["hadolint", "typos"],
        "typecheck": [],
        "unused": [],
    },
    # flang typecheck stays opt-in: it replays every compile action (heavy tier).
    "fortran": {
        "audit": [],
        "format": ["ffmt"],
        "lint": ["fortitude", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "go": {
        "audit": [],
        "format": ["gofumpt"],
        "lint": ["govet", "staticcheck", "typos"],
        "typecheck": ["gotype"],
        "unused": ["deadcode"],
    },
    "go_module": {
        "audit": [],
        "format": ["modfmt"],
        "lint": ["typos"],
        "typecheck": [],
        "unused": [],
    },
    "graphql": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": [],
        "unused": [],
    },
    # ghc typecheck stays opt-in: it replays every compile action (heavy tier).
    "haskell": {
        "audit": [],
        "format": ["ormolu"],
        "lint": ["hlint", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "html": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": [],
        "unused": [],
    },
    "java": {
        "audit": [],
        "format": ["google_java_format"],
        "lint": ["checkstyle", "pmd", "spotbugs", "typos"],
        "typecheck": ["javac"],
        "unused": [],
    },
    "javascript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "json": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["biome"],
        "typecheck": [],
        "unused": [],
    },
    "julia": {
        "audit": [],
        "format": [],
        "lint": ["julialint", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "kotlin": {
        "audit": [],
        "format": ["ktfmt"],
        "lint": ["detekt", "ktlint", "typos"],
        "typecheck": ["kotlinc"],
        "unused": [],
    },
    "helm": {
        "audit": [],
        "format": [],
        "lint": ["helm", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "kubernetes": {
        "audit": [],
        "format": [],
        "lint": ["kubeconform", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "markdown": {
        "audit": [],
        "format": [],
        "lint": ["lychee", "markdown_check", "typos", "vale"],
        "typecheck": [],
        "unused": [],
    },
    "mdx": {
        "audit": [],
        "format": ["remark"],
        "lint": ["typos"],
        "typecheck": ["tsc"],
        "unused": [],
    },
    # nixfmt is pinned for linux_x86_64 only, so nix has no formatter (#8045).
    "nix": {
        "audit": [],
        "format": [],
        "lint": ["deadnix", "statix", "typos"],
        "typecheck": [],
        "unused": [],
    },
    # ocaml typecheck stays opt-in: it replays every compile action (heavy tier).
    "ocaml": {
        "audit": [],
        "format": ["topiary"],
        "lint": ["typos", "zanuda"],
        "typecheck": [],
        "unused": [],
    },
    "powershell": {
        "audit": [],
        "format": ["psscriptanalyzer"],
        "lint": ["psscriptanalyzer", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "python": {
        "audit": [],
        "format": ["ruff"],
        "lint": ["pydoclint", "ruff", "typos"],
        "typecheck": ["ty"],
        "unused": ["vulture"],
    },
    "rust": {
        "audit": [],
        "format": ["rustfmt"],
        "lint": ["clippy", "typos"],
        "typecheck": ["rustc"],
        "unused": [],
    },
    "scala": {
        "audit": [],
        "format": ["scalafmt"],
        "lint": ["typos"],
        "typecheck": ["scalac"],
        "unused": [],
    },
    "shell": {
        "audit": [],
        "format": ["shfmt"],
        "lint": ["shellcheck", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "starlark": {
        "audit": [],
        "format": ["buildifier"],
        "lint": ["buildifier", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "svelte": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": ["tsc"],
        "unused": [],
    },
    "swift": {
        "audit": [],
        "format": ["swift_format"],
        "lint": ["swift_lint", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "text": {
        "audit": [],
        "format": [],
        "lint": ["keep_sorted", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "toml": {
        "audit": [],
        "format": ["taplo"],
        "lint": ["taplo", "typos"],
        "typecheck": [],
        "unused": [],
    },
    "typescript": {
        "audit": [],
        "format": ["biome"],
        "lint": ["biome", "typos"],
        "typecheck": ["tsc"],
        "unused": [],
    },
    "vue": {
        "audit": [],
        "format": ["prettier"],
        "lint": ["typos"],
        "typecheck": ["tsc"],
        "unused": [],
    },
    "zig": {
        "audit": [],
        "format": ["zig_fmt"],
        "lint": ["typos", "zig_ast_check"],
        "typecheck": [],
        "unused": [],
    },
}

# Tools that several families select. They always use the root profile, so a family
# profile never changes how they run.
CROSS_FAMILY_TOOLS = [
    "biome",
    "lychee",
    "markdown_check",
    "prettier",
    "tsc",
    "typos",
]

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
        if sorted(entry.keys()) != ["audit", "format", "lint", "typecheck", "unused"]:
            return "curated defaults: family '" + family + "' must carry exactly audit/format/lint/typecheck/unused"
        for capability in entry:
            for tool in entry[capability]:
                if not _is_canonical_token(tool):
                    return "curated defaults: non-canonical tool '" + str(tool) + "' for family '" + family + "'"
    return ""
