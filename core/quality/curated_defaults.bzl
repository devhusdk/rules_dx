"""Curated quality defaults."""

load(":tool_defs.bzl", "is_canonical_token")

# Julia lints with julialint only. Its extra fetch over typos is the dx_julia_lint package set
# (21 packages, about 4.2 MB); the 1.12.5 runtime (about 288 MB on linux_x86_64) is already
# fetched to precompile any julia_library (#8048). JET and JuliaFormatter stay opt-in.
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
        "lint": ["clang_tidy", "typos"],
        "typecheck": [],
    },
    "cloudformation": {
        "audit": [],
        "format": [],
        "lint": ["cfn_lint", "typos"],
        "typecheck": [],
    },
    "core": {
        "audit": [],
        "format": ["yamlfmt"],
        "lint": ["actionlint", "zizmor"],
        "typecheck": [],
    },
    "d": {
        "audit": [],
        "format": [],
        "lint": ["typos"],
        "typecheck": [],
    },
    "dart": {
        "audit": [],
        "format": ["dart"],
        "lint": ["dart", "typos"],
        "typecheck": ["dart"],
    },
    "dockerfile": {
        "audit": [],
        "format": [],
        "lint": ["hadolint", "typos"],
        "typecheck": [],
    },
    # flang typecheck stays opt-in: it replays every compile action (heavy tier).
    "fortran": {
        "audit": [],
        "format": ["ffmt"],
        "lint": ["fortitude", "typos"],
        "typecheck": [],
    },
    "go": {
        "audit": [],
        "format": ["gofumpt"],
        "lint": ["errcheck", "govet", "staticcheck", "typos"],
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
    # ghc typecheck stays opt-in: it replays every compile action (heavy tier).
    "haskell": {
        "audit": [],
        "format": [],
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
        "lint": ["pmd", "spotbugs", "typos"],
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
        "format": ["biome"],
        "lint": ["biome"],
        "typecheck": [],
    },
    "julia": {
        "audit": [],
        "format": [],
        "lint": ["julialint", "typos"],
        "typecheck": [],
    },
    "kotlin": {
        "audit": [],
        "format": ["ktfmt"],
        "lint": ["detekt", "typos"],
        "typecheck": ["kotlinc"],
    },
    # helmfmt is pre-1.0 and prettier skips chart templates, so helm format stays opt-in.
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
        "lint": ["lychee", "markdown_check", "typos"],
        "typecheck": [],
    },
    "mdx": {
        "audit": [],
        "format": ["remark"],
        "lint": ["lychee", "typos"],
        "typecheck": ["tsc"],
    },
    # nixfmt does not build on Windows, so nix has no curated formatter (#8045).
    "nix": {
        "audit": [],
        "format": [],
        "lint": ["deadnix", "typos"],
        "typecheck": [],
    },
    # ocaml typecheck stays opt-in: it replays every compile action (heavy tier).
    "ocaml": {
        "audit": [],
        "format": ["topiary"],
        "lint": ["typos"],
        "typecheck": [],
    },
    "oci_image": {
        "audit": [],
        "format": [],
        "lint": ["trivy_image_config"],
        "typecheck": [],
    },
    "powershell": {
        "audit": [],
        "format": [],
        "lint": ["typos"],
        "typecheck": [],
    },
    "python": {
        "audit": [],
        "format": ["ruff"],
        "lint": ["ruff", "typos"],
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
    "swift": {
        "audit": [],
        "format": [],
        "lint": ["typos"],
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
    "yaml": {
        "audit": [],
        "format": ["yamlfmt"],
        "lint": ["yamllint"],
        "typecheck": [],
    },
    "zig": {
        "audit": [],
        "format": ["zig_fmt"],
        "lint": ["typos", "zig_ast_check"],
        "typecheck": [],
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
