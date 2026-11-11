"""The consumer fixtures and the test of each."""

load("//rust:defs.bzl", "rust_test")

CONSUMER_FIXTURES = [
    "csharp",
    "d",
    "elixir",
    "erlang",
    "go",
    "haskell",
    "java",
    "julia",
    "kotlin",
    "lua",
    "ocaml",
    "perl",
    "qt",
    "php",
    "rego",
    "ruby",
    "rust",
    "scala",
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
                "@dx_test_tags_sources//:consumer.json",
            ],
            env = {
                "DX_BIN": "$(rootpath //cli/cli:dx)",
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
