"""Versioned semantic-class registry tests (freeze)."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":sources.bzl", "RUST", "SOURCES_REGISTRY_SCHEMA_VERSION", "is_known_semantic_class", "sources_schema_error", "upstream_source_class")

FROZEN_SEMANTIC_FILE_CLASSES = [
    "text",
    "c",
    "cpp",
    "cuda",
    "csharp",
    "fsharp",
    "powershell",
    "css",
    "less",
    "scss",
    "javascript",
    "jsx",
    "typescript",
    "tsx",
    "vue",
    "svelte",
    "astro",
    "mdx",
    "graphql",
    "html",
    "html_template",
    "json",
    "json5",
    "jsonc",
    "markdown",
    "toml",
    "xml",
    "yaml",
    "java",
    "kotlin",
    "scala",
    "python",
    "python_stub",
    "cue",
    "gherkin",
    "go",
    "jsonnet",
    "pkl",
    "protobuf",
    "qml",
    "ruby",
    "rust",
    "shell",
    "sql",
    "starlark",
    "terraform",
    "go_module",
]

def sources_registry_tests(name):
    starlark_test(
        name = name,
        mode = "load",
        checks = [
            expect_equal(
                "sources schema version stays v1",
                SOURCES_REGISTRY_SCHEMA_VERSION,
                1,
            ),
            expect_equal(
                "sources registry schema validates",
                sources_schema_error(),
                "",
            ),
            expect_equal(
                "frozen core IDs stay known (additions need no allowlist edit)",
                [c for c in FROZEN_SEMANTIC_FILE_CLASSES if not is_known_semantic_class(c)],
                [],
            ),
            expect_equal("RUST class ID", RUST, "rust"),
            expect_equal(
                "upstream rust extension maps to rust",
                upstream_source_class("rs"),
                "rust",
            ),
            expect_equal(
                "upstream c and h extensions map to c",
                [upstream_source_class("c"), upstream_source_class("h")],
                ["c", "c"],
            ),
            expect_equal(
                "upstream cxx extensions map to cpp",
                [upstream_source_class("cc"), upstream_source_class("cpp"), upstream_source_class("cxx"), upstream_source_class("hh"), upstream_source_class("hpp"), upstream_source_class("hxx")],
                ["cpp", "cpp", "cpp", "cpp", "cpp", "cpp"],
            ),
            expect_equal(
                "upstream cuda extensions map to cuda",
                [upstream_source_class("cu"), upstream_source_class("cuh")],
                ["cuda", "cuda"],
            ),
            expect_equal(
                "upstream unknown extension maps to empty",
                [upstream_source_class("py"), upstream_source_class("")],
                ["", ""],
            ),
        ],
    )
