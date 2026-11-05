"""Unit tests for the curated defaults."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":curated_defaults.bzl", "CURATED_DEFAULTS", "curated_schema_error")

def curated_defaults_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "curated manifest schema validates",
                curated_schema_error(),
                "",
            ),
            expect_equal(
                "python curated defaults stay Ruff + Ty + pydoclint",
                CURATED_DEFAULTS["python"],
                {
                    "audit": [],
                    "format": ["ruff"],
                    "lint": ["pydoclint", "ruff", "typos"],
                    "typecheck": ["ty"],
                    "unused": ["vulture"],
                },
            ),
            expect_equal(
                "typescript curated defaults are Biome plus tsc typecheck",
                CURATED_DEFAULTS["typescript"],
                {
                    "audit": [],
                    "format": ["biome"],
                    "lint": ["biome", "typos"],
                    "typecheck": ["tsc"],
                    "unused": [],
                },
            ),
            expect_equal(
                "javascript curated defaults stay Biome-only",
                CURATED_DEFAULTS["javascript"],
                {
                    "audit": [],
                    "format": ["biome"],
                    "lint": ["biome", "typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "json curated defaults stay Biome lint + Prettier format",
                CURATED_DEFAULTS["json"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": ["biome"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "graphql curated defaults stay Prettier format only",
                CURATED_DEFAULTS["graphql"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": ["typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "html curated defaults stay Prettier format only",
                CURATED_DEFAULTS["html"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": ["typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "astro curated defaults are Prettier format and tsc typecheck",
                CURATED_DEFAULTS["astro"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": ["typos"],
                    "typecheck": ["tsc"],
                    "unused": [],
                },
            ),
            expect_equal(
                "mdx curated defaults are remark format plus tsc typecheck",
                CURATED_DEFAULTS["mdx"],
                {
                    "audit": [],
                    "format": ["remark"],
                    "lint": ["typos"],
                    "typecheck": ["tsc"],
                    "unused": [],
                },
            ),
            expect_equal(
                "svelte curated defaults are Prettier format and tsc typecheck",
                CURATED_DEFAULTS["svelte"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": ["typos"],
                    "typecheck": ["tsc"],
                    "unused": [],
                },
            ),
            expect_equal(
                "vue curated defaults are Prettier format plus tsc typecheck",
                CURATED_DEFAULTS["vue"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": ["typos"],
                    "typecheck": ["tsc"],
                    "unused": [],
                },
            ),
            expect_equal(
                "dockerfile curated defaults stay hadolint lint only (check-only, no formatter)",
                CURATED_DEFAULTS["dockerfile"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["hadolint", "typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "rust curated defaults stay rustfmt + Clippy + rustc",
                CURATED_DEFAULTS["rust"],
                {
                    "audit": [],
                    "format": ["rustfmt"],
                    "lint": ["clippy", "typos"],
                    "typecheck": ["rustc"],
                    "unused": [],
                },
            ),
            expect_equal(
                "haskell curated defaults are ormolu + hlint",
                CURATED_DEFAULTS["haskell"],
                {
                    "audit": [],
                    "format": ["ormolu"],
                    "lint": ["hlint", "typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "d curated defaults are dfmt + D-Scanner",
                CURATED_DEFAULTS["d"],
                {
                    "audit": [],
                    "format": ["dfmt"],
                    "lint": ["dscanner", "typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "dart curated defaults are dart format + analyze",
                CURATED_DEFAULTS["dart"],
                {
                    "audit": [],
                    "format": ["dart"],
                    "lint": ["dart", "typos"],
                    "typecheck": ["dart"],
                    "unused": [],
                },
            ),
            expect_equal(
                "nix curated defaults are lint only until nixfmt is multi-platform",
                CURATED_DEFAULTS["nix"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["deadnix", "statix", "typos"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
            expect_equal(
                "zig curated defaults are zig fmt + ast-check",
                CURATED_DEFAULTS["zig"],
                {
                    "audit": [],
                    "format": ["zig_fmt"],
                    "lint": ["typos", "zig_ast_check"],
                    "typecheck": [],
                    "unused": [],
                },
            ),
        ],
    )
