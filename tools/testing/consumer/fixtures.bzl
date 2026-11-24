"""The consumer fixtures and the test of each."""

load("//dx:generate.bzl", "GENERATE_EXCLUDES")
load("//rust:defs.bzl", "rust_test")

CONSUMER_FIXTURES = [
    "ansible",
    "android",
    "api_spec",
    "appimage",
    "apple",
    "asciidoc",
    "astro",
    "avro",
    "bicep",
    "capnproto",
    "cc",
    "cloudformation",
    "cmake",
    "csharp",
    "cuda",
    "d",
    "dart",
    "dockerfile",
    "elixir",
    "erlang",
    "flatbuffers",
    "flutter",
    "fortran",
    "fsharp",
    "go",
    "haskell",
    "helm",
    "java",
    "javascript",
    "julia",
    "kotlin",
    "kubernetes",
    "lua",
    "make",
    "mdx",
    "msix",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "python",
    "qt",
    "rego",
    "rst",
    "ruby",
    "rust",
    "scala",
    "sql",
    "svelte",
    "swift",
    "terraform",
    "thrift",
    "typescript",
    "vue",
    "zig",
]

GO_GUARD_LANGS = [
    "go",
    "lua",
    "capnproto",
    "flatbuffers",
    "thrift",
    "api_spec",
    "avro",
    "rust",
    "python",
    "cc",
]

JAVA_GUARD_LANGS = [
    "java",
]

IN_GRAPH_GUARD_LANGS = [
    "android",
    "ansible",
    "appimage",
    "asciidoc",
    "astro",
    "bicep",
    "cloudformation",
    "cmake",
    "csharp",
    "cuda",
    "d",
    "dart",
    "dockerfile",
    "elixir",
    "erlang",
    "flutter",
    "fortran",
    "fsharp",
    "haskell",
    "helm",
    "javascript",
    "julia",
    "kotlin",
    "kubernetes",
    "make",
    "mdx",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "qt",
    "rego",
    "rst",
    "ruby",
    "scala",
    "sql",
    "svelte",
    "terraform",
    "typescript",
    "vue",
    "zig",
]

MACOS_IN_GRAPH_GUARD_LANGS = [
    "apple",
    "swift",
]

WINDOWS_IN_GRAPH_GUARD_LANGS = [
    "msix",
]

GUARD_PENDING = []

CONSUMER_ROWS = [
    "android",
    "astro",
    "cc",
    "csharp",
    "cuda",
    "d",
    "dart",
    "elixir",
    "erlang",
    "fortran",
    "fsharp",
    "go",
    "haskell",
    "java",
    "javascript",
    "julia",
    "lua",
    "mdx",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "python",
    "ruby",
    "rust",
    "svelte",
    "typescript",
    "vue",
    "zig",
]

def consumer_fixture_tests(name, size):
    """Declares one test per consumer fixture that runs dx init on a copy of its sources, and a suite of them."""
    tests = []
    for lang in CONSUMER_FIXTURES:
        test = lang + "_consumer_test"
        tests.append(":" + test)
        rust_test(
            name = test,
            size = size,
            srcs = ["fixture_test.rs"],
            crate_name = "consumer_fixture_test",
            data = [
                "pending.txt",
                "//cli/cli:dx",
                "//cli/cli/tests/fake_generate",
                "@dx_gazelle//:gazelle",
                "@dx_test_tags_sources//:consumer.json",
            ],
            env = {
                "DX_BIN": "$(rootpath //cli/cli:dx)",
                "DX_FAKE_GENERATE": "$(rootpath //cli/cli/tests/fake_generate)",
                "DX_GAZELLE_BIN": "$(rootpath @dx_gazelle//:gazelle)",
                "DX_FAKE_GAZELLE_ARGS": " ".join(GENERATE_EXCLUDES),
                "DX_CONSUMER_JSON": "$(rlocationpath @dx_test_tags_sources//:consumer.json)",
                "DX_CONSUMER_LANG": lang,
                "DX_CONSUMER_PENDING": "$(rootpath pending.txt)",
            },
            tags = ["consumer-row:" + lang] if lang in CONSUMER_ROWS else [],
            deps = ["//tools/testing:dx_testing"],
        )
    native.test_suite(
        name = name,
        tests = tests,
    )
