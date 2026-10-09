"""JavaScript/TypeScript/JSON tool-responsibility gate (freeze)."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")
load(":curated_defaults.bzl", "CURATED_DEFAULTS")
load(":parity_tests.bzl", "PARITY_DEFERRED")
load(":pipeline.bzl", "pipeline_stages", "resolve_pipeline")

_JS_FAMILIES = ["javascript", "json", "typescript"]

_JS_TOOLS = ["biome", "eslint", "prettier"]

_DEFAULT_FORMAT_SELECTIONS = {family: CURATED_DEFAULTS[family]["format"] for family in _JS_FAMILIES}

_DEFAULT_LINT_SELECTIONS = {family: CURATED_DEFAULTS[family]["lint"] for family in _JS_FAMILIES}

_DIRECT_SOURCES = {
    "javascript": ["src/app.js"],
    "json": ["config/data.json"],
    "jsx": ["src/view.jsx"],
    "tsx": ["src/app.tsx"],
    "typescript": ["src/main.ts"],
}

def _claimants(capability, class_id):
    """Returns the sorted tools claiming one class/capability from the registry."""
    return sorted([tool for tool in REAL_ADAPTERS if class_id in REAL_ADAPTERS[tool].get(capability, [])])

def _js_tool_classes():
    """Returns the sorted classes any JS tool claims in any capability."""
    seen = {}
    for tool in _JS_TOOLS:
        for capability in REAL_ADAPTERS[tool]:
            for class_id in REAL_ADAPTERS[tool][capability]:
                seen[class_id] = True
    return sorted(seen.keys())

def js_responsibilities_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "biome and prettier both claim javascript format",
                _claimants("format", "javascript"),
                ["biome", "prettier"],
            ),
            expect_equal(
                "biome and prettier both claim jsx format",
                _claimants("format", "jsx"),
                ["biome", "prettier"],
            ),
            expect_equal(
                "biome and prettier both claim typescript format",
                _claimants("format", "typescript"),
                ["biome", "prettier"],
            ),
            expect_equal(
                "biome and prettier both claim tsx format",
                _claimants("format", "tsx"),
                ["biome", "prettier"],
            ),
            expect_equal(
                "biome and prettier both claim json format",
                _claimants("format", "json"),
                ["biome", "prettier"],
            ),
            expect_equal(
                "biome and eslint share javascript lint; typescript stays biome-only",
                [
                    _claimants("lint", "javascript"),
                    _claimants("lint", "jsx"),
                    _claimants("lint", "typescript"),
                    _claimants("lint", "tsx"),
                    _claimants("lint", "json"),
                ],
                [
                    ["biome", "eslint"],
                    ["biome", "eslint"],
                    ["biome"],
                    ["biome"],
                    ["biome"],
                ],
            ),
            expect_equal(
                "prettier never lints and eslint never formats js/ts/json",
                [
                    sorted([c for c in ["javascript", "json", "jsx", "tsx", "typescript"] if "prettier" in _claimants("lint", c)]),
                    sorted([c for c in ["javascript", "json", "jsx", "tsx", "typescript"] if "eslint" in _claimants("format", c)]),
                ],
                [[], []],
            ),
            expect_equal(
                "tsc alone typechecks typescript and tsx",
                [
                    _claimants("typecheck", "typescript"),
                    _claimants("typecheck", "tsx"),
                    _claimants("typecheck", "javascript"),
                ],
                [["tsc"], ["tsc"], []],
            ),
            expect_equal(
                "deferred framework classes stay outside JS tool claims",
                [c for c in sorted(PARITY_DEFERRED.keys()) if c in _js_tool_classes()],
                [],
            ),
            expect_equal(
                "default format selections stay single-formatter per family",
                _DEFAULT_FORMAT_SELECTIONS,
                {
                    "javascript": ["biome"],
                    "json": ["prettier"],
                    "typescript": ["biome"],
                },
            ),
            expect_equal(
                "default lint selections stay biome-only per family",
                _DEFAULT_LINT_SELECTIONS,
                {
                    "javascript": ["biome"],
                    "json": ["biome"],
                    "typescript": ["biome"],
                },
            ),
            expect_equal(
                "default format resolves to one stage per file, never composed",
                pipeline_stages(
                    ["javascript", "jsx", "typescript", "tsx", "json"],
                    "format",
                    _DEFAULT_FORMAT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript", "jsx", "tsx", "typescript"], "tool": "biome"},
                    {"classes": ["json"], "tool": "prettier"},
                ],
            ),
            expect_equal(
                "default lint resolves to one biome stage with no eslint",
                pipeline_stages(
                    ["javascript", "jsx", "typescript", "tsx", "json"],
                    "lint",
                    _DEFAULT_LINT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript", "json", "jsx", "tsx", "typescript"], "tool": "biome"},
                ],
            ),
            expect_equal(
                "default format carries exact per-stage source subsets",
                resolve_pipeline(
                    ["javascript", "jsx", "typescript", "tsx", "json"],
                    _DIRECT_SOURCES,
                    "format",
                    _DEFAULT_FORMAT_SELECTIONS,
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {
                        "classes": ["javascript", "jsx", "tsx", "typescript"],
                        "sources": ["src/app.js", "src/app.tsx", "src/main.ts", "src/view.jsx"],
                        "tool": "biome",
                    },
                    {
                        "classes": ["json"],
                        "sources": ["config/data.json"],
                        "tool": "prettier",
                    },
                ],
            ),
            expect_equal(
                "consumer prettier selection formats javascript without biome",
                pipeline_stages(
                    ["javascript"],
                    "format",
                    {"javascript": ["prettier"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript"], "tool": "prettier"},
                ],
            ),
            expect_equal(
                "consumer biome selection formats json without prettier",
                pipeline_stages(
                    ["json"],
                    "format",
                    {"json": ["biome"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["json"], "tool": "biome"},
                ],
            ),
            expect_equal(
                "consumer eslint selection lints javascript without biome",
                pipeline_stages(
                    ["javascript", "jsx"],
                    "lint",
                    {"javascript": ["eslint"]},
                    REAL_CLASS_TO_FAMILY,
                    REAL_ADAPTERS,
                ),
                [
                    {"classes": ["javascript", "jsx"], "tool": "eslint"},
                ],
            ),
        ],
    )
