"""Unit tests for the curated defaults."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_CLASS_TO_FAMILY")
load(":curated_defaults.bzl", "CURATED_DEFAULTS", "LANG_JSON_DEFAULTS", "OTHER_DEFAULTS", "curated_schema_error")
load(":tools.bzl", "TOOLS")

visibility("private")

_OPT_IN = "opt-in family with no curated defaults"

# Families whose default lint is typos alone or that have no defaults, and why. Remove an
# entry when the family gets a real default linter; never add one.
LINT_DEBT = {
    "ansible": _OPT_IN,
    "api_spec": _OPT_IN,
    "asciidoc": _OPT_IN,
    "astro": "no Astro linter is pinned",
    "avro": _OPT_IN,
    "capnp": _OPT_IN,
    "cmake": _OPT_IN,
    "csharp": _OPT_IN,
    "css": _OPT_IN,
    "cue": _OPT_IN,
    "elixir": _OPT_IN,
    "erlang": _OPT_IN,
    "flatbuffers": _OPT_IN,
    "fsharp": _OPT_IN,
    "gherkin": _OPT_IN,
    "go_module": "modfmt formats go.mod; no go.mod linter is pinned",
    "graphql": "graphql_eslint is opt-in",
    "haskell": "hlint is opt-in",
    "html": "no plain HTML linter is pinned",
    "html_template": _OPT_IN,
    "jsonnet": _OPT_IN,
    "jupyter": _OPT_IN,
    "latex": _OPT_IN,
    "lua": _OPT_IN,
    "make": _OPT_IN,
    "objc": "clang_tidy is opt-in for Objective-C",
    "ocaml": "zanuda is opt-in",
    "perl": _OPT_IN,
    "php": _OPT_IN,
    "pkl": _OPT_IN,
    "powershell": "psscriptanalyzer is opt-in",
    "protobuf": _OPT_IN,
    "qml": _OPT_IN,
    "rego": _OPT_IN,
    "rst": _OPT_IN,
    "ruby": _OPT_IN,
    "scala": "scalafix is opt-in",
    "sql": _OPT_IN,
    "svelte": "no Svelte linter is pinned",
    "swift": "swift_lint is opt-in",
    "terraform": _OPT_IN,
    "thrift": _OPT_IN,
    "typst": _OPT_IN,
    "vue": "no Vue linter is pinned",
    "xml": _OPT_IN,
}

def _families():
    found = {family: True for family in CURATED_DEFAULTS}
    for tool in TOOLS:
        for classes in tool.adapters.values():
            for class_id in classes:
                found[REAL_CLASS_TO_FAMILY[class_id]] = True
    return sorted(found.keys())

def _lint_debt():
    """Returns the families with no default linter besides typos."""
    return [
        family
        for family in _families()
        if not [tool for tool in CURATED_DEFAULTS.get(family, {"lint": []})["lint"] if tool != "typos"]
    ]

def curated_defaults_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "every family's default lint holds more than typos, or is lint debt",
                [family for family in _lint_debt() if family not in LINT_DEBT],
                [],
            ),
            expect_equal(
                "every lint debt entry is still needed",
                [family for family in LINT_DEBT if family not in _lint_debt()],
                [],
            ),
            expect_equal(
                "curated manifest schema validates",
                curated_schema_error(),
                "",
            ),
            expect_equal(
                "lang.json curated rows and the other rows do not overlap",
                [family for family in LANG_JSON_DEFAULTS if family in OTHER_DEFAULTS],
                [],
            ),
            expect_equal(
                "curated defaults hold every lang.json and other row",
                len(CURATED_DEFAULTS),
                len(LANG_JSON_DEFAULTS) + len(OTHER_DEFAULTS),
            ),
            expect_equal(
                "cuda curated defaults come from lang.json",
                ["cuda" in LANG_JSON_DEFAULTS, CURATED_DEFAULTS["cuda"]],
                [True, {
                    "audit": [],
                    "format": ["clang_format"],
                    "lint": ["clang_tidy", "typos"],
                    "typecheck": [],
                }],
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
                "d curated defaults lint with dscanner and typos",
                CURATED_DEFAULTS["d"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["dscanner", "typos"],
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
                "nix curated defaults are deadnix lint only; nixfmt does not build on Windows",
                CURATED_DEFAULTS["nix"],
                {
                    "audit": [],
                    "format": [],
                    "lint": ["deadnix", "typos"],
                    "typecheck": [],
                },
            ),
            expect_equal(
                "swift curated defaults are typos only; swift-format and swift_lint are opt-in",
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
                "objc curated defaults are clang-format, typos and clang_syntax",
                (CURATED_DEFAULTS["objc"], "objc" in OTHER_DEFAULTS),
                (
                    {
                        "audit": [],
                        "format": ["clang_format"],
                        "lint": ["typos"],
                        "typecheck": ["clang_syntax"],
                    },
                    True,
                ),
            ),
            expect_equal(
                "java curated defaults",
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
