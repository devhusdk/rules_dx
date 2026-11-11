"""The consumer fixtures and the test of each."""

load("//dx:generate.bzl", "GENERATE_EXCLUDES")
load("//rust:defs.bzl", "rust_test")

CONSUMER_FIXTURES = [
    "csharp",
    "cuda",
    "d",
    "dart",
    "elixir",
    "erlang",
    "fsharp",
    "go",
    "haskell",
    "java",
    "julia",
    "kotlin",
    "lua",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "qt",
    "rego",
    "ruby",
    "rust",
    "scala",
    "swift",
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
            deps = ["//tools/testing:dx_testing"],
        )
    native.test_suite(
        name = name,
        tests = tests,
    )
