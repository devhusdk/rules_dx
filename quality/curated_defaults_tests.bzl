"""Unit tests for the versioned curated defaults."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":curated_defaults.bzl", "CURATED_DEFAULTS", "CURATED_SCHEMA_VERSION", "FORMAT_FROZEN", "curated_families", "curated_schema_error")

FROZEN_CURATED_FAMILIES = [
    "javascript",
    "json",
    "markdown",
    "python",
    "rust",
    "starlark",
    "toml",
    "typescript",
]

def curated_defaults_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "curated schema version stays v1",
                CURATED_SCHEMA_VERSION,
                1,
            ),
            expect_equal(
                "curated manifest schema validates",
                curated_schema_error(),
                "",
            ),
            expect_equal(
                "frozen curated families stay curated (additions need no allowlist edit)",
                [f for f in FROZEN_CURATED_FAMILIES if f not in curated_families()],
                [],
            ),
            expect_equal(
                "python curated defaults stay Ruff + Ty + pydoclint",
                CURATED_DEFAULTS["python"],
                {
                    "audit": [],
                    "format": ["ruff"],
                    "lint": ["pydoclint", "ruff"],
                    "typecheck": ["ty"],
                },
            ),
            expect_equal(
                "typescript curated defaults stay Biome + tsc",
                CURATED_DEFAULTS["typescript"],
                {
                    "audit": [],
                    "format": ["biome"],
                    "lint": ["biome"],
                    "typecheck": ["tsc"],
                },
            ),
            expect_equal(
                "javascript curated defaults stay Biome-only",
                CURATED_DEFAULTS["javascript"],
                {
                    "audit": [],
                    "format": ["biome"],
                    "lint": ["biome"],
                    "typecheck": [],
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
                },
            ),
            expect_equal(
                "graphql curated defaults stay Prettier format only",
                CURATED_DEFAULTS["graphql"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": [],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "html curated defaults stay Prettier format only",
                CURATED_DEFAULTS["html"],
                {
                    "audit": [],
                    "format": ["prettier"],
                    "lint": [],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "rust curated defaults stay rustfmt + Clippy + rustc",
                CURATED_DEFAULTS["rust"],
                {
                    "audit": [],
                    "format": ["rustfmt"],
                    "lint": ["clippy"],
                    "typecheck": ["rustc"],
                },
            ),
            expect_equal(
                "formatter set stays frozen for the core families (major-release gate)",
                {f: FORMAT_FROZEN[f] for f in FROZEN_CURATED_FAMILIES},
                {
                    "javascript": ["biome"],
                    "json": ["prettier"],
                    "markdown": [],
                    "python": ["ruff"],
                    "rust": ["rustfmt"],
                    "starlark": ["buildifier"],
                    "toml": ["taplo"],
                    "typescript": ["biome"],
                },
            ),
        ],
    )
