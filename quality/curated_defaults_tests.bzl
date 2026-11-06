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
                "python curated defaults are Ruff + Ty",
                CURATED_DEFAULTS["python"],
                {
                    "audit": [],
                    "format": ["ruff"],
                    "lint": ["ruff", "typos"],
                    "typecheck": ["ty"],
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
                },
            ),
            expect_equal(
                "json curated defaults are Biome format and lint",
                CURATED_DEFAULTS["json"],
                {
                    "audit": [],
                    "format": ["biome"],
                    "lint": ["biome"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "julia curated defaults lint with julialint and typos; JET and JuliaFormatter stay opt-in",
                CURATED_DEFAULTS["julia"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["julialint", "typos"],
                    "typecheck": [],
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
                },
            ),
            expect_equal(
                "mdx curated defaults are remark format, lychee links, tsc typecheck",
                CURATED_DEFAULTS["mdx"],
                {
                    "audit": [],
                    "format": ["remark"],
                    "lint": ["lychee", "typos"],
                    "typecheck": ["tsc"],
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
                },
            ),
            expect_equal(
                "haskell curated defaults are typos only; hlint and ormolu are catalog tools",
                CURATED_DEFAULTS["haskell"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "fortran curated defaults are ffmt + fortitude",
                CURATED_DEFAULTS["fortran"],
                {
                    "audit": [],
                    "format": ["ffmt"],
                    "lint": ["fortitude", "typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "ocaml curated defaults are topiary format; zanuda is catalog (linux_x86_64 only)",
                CURATED_DEFAULTS["ocaml"],
                {
                    "audit": [],
                    "format": ["topiary"],
                    "lint": ["typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "powershell curated defaults are typos only; psscriptanalyzer has no Windows build",
                CURATED_DEFAULTS["powershell"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "d curated defaults are typos only",
                CURATED_DEFAULTS["d"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["typos"],
                    "typecheck": [],
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
                },
            ),
            expect_equal(
                "nix curated defaults are deadnix lint only; nixfmt is single-platform",
                CURATED_DEFAULTS["nix"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["deadnix", "typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "swift curated defaults are typos only; swift-format and swiftlint have no macos_arm64 build",
                CURATED_DEFAULTS["swift"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["typos"],
                    "typecheck": [],
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
                },
            ),
            expect_equal(
                "go curated defaults are gofumpt, govet, staticcheck, errcheck and gotype",
                CURATED_DEFAULTS["go"],
                {
                    "audit": [],
                    "format": ["gofumpt"],
                    "lint": ["errcheck", "govet", "staticcheck", "typos"],
                    "typecheck": ["gotype"],
                },
            ),
            expect_equal(
                "cc curated defaults are clang-format and clang-tidy",
                CURATED_DEFAULTS["cc"],
                {
                    "audit": [],
                    "format": ["clang_format"],
                    "lint": ["clang_tidy", "typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "java curated defaults drop checkstyle",
                CURATED_DEFAULTS["java"],
                {
                    "audit": [],
                    "format": ["google_java_format"],
                    "lint": ["pmd", "spotbugs", "typos"],
                    "typecheck": ["javac"],
                },
            ),
            expect_equal(
                "kotlin curated defaults",
                CURATED_DEFAULTS["kotlin"],
                {
                    "audit": [],
                    "format": ["ktfmt"],
                    "lint": ["detekt", "typos"],
                    "typecheck": ["kotlinc"],
                },
            ),
            expect_equal(
                "markdown curated defaults leave links to lychee and drop vale",
                CURATED_DEFAULTS["markdown"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["lychee", "markdown_check", "typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "yaml curated defaults are yamlfmt and yamllint",
                CURATED_DEFAULTS["yaml"],
                {
                    "audit": [],
                    "format": ["yamlfmt"],
                    "lint": ["yamllint"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "core curated defaults cover workflows with yamlfmt, actionlint and zizmor",
                CURATED_DEFAULTS["core"],
                {
                    "audit": [],
                    "format": ["yamlfmt"],
                    "lint": ["actionlint", "zizmor"],
                    "typecheck": [],
                },
            ),
        ],
    )
