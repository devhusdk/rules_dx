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

# The consumer fixtures //tools/testing/consumer:go_prebuilt_guard_test covers.
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

# The consumer fixtures //tools/testing/consumer:java_prebuilt_guard_test covers.
JAVA_GUARD_LANGS = [
    "java",
]

# The consumer fixtures //tools/testing/consumer:in_graph_prebuilt_guard covers.
IN_GRAPH_GUARD_LANGS = [
    "d",
    "kotlin",
    "rego",
    "scala",
    "sql",
    "zig",
]

# The consumer fixtures no prebuilt guard covers yet.
GUARD_PENDING = [
    "ansible",
    "android",
    "appimage",
    "apple",
    "asciidoc",
    "astro",
    "bicep",
    "cloudformation",
    "cmake",
    "csharp",
    "cuda",
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
    "kubernetes",
    "make",
    "mdx",
    "msix",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "qt",
    "rst",
    "ruby",
    "svelte",
    "swift",
    "terraform",
    "typescript",
    "vue",
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
            deps = ["//tools/testing:dx_testing"],
        )
    native.test_suite(
        name = name,
        tests = tests,
    )
