"""Unit tests for source-fixture shape."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":fixtures.bzl", "markdown_sibling_error")

def fixtures_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "markdown_sibling_error rejects siblings without markdown sources",
                markdown_sibling_error(
                    "//q:corpus_markdown",
                    ["doc.proto", "//q/ir:src/lib.rs"],
                    [],
                ),
                "real_source_target (//q:corpus_markdown): markdown_siblings " +
                "needs markdown_srcs; siblings are link targets for checked " +
                "markdown, never sources",
            ),
            expect_equal(
                "markdown_sibling_error accepts siblings beside markdown sources",
                markdown_sibling_error(
                    "//q:corpus_markdown",
                    ["//q:README.md"],
                    ["README.md"],
                ),
                "",
            ),
            expect_equal(
                "markdown_sibling_error accepts markdown sources alone",
                markdown_sibling_error("//q:corpus_markdown", [], ["README.md"]),
                "",
            ),
            expect_equal(
                "markdown_sibling_error accepts a target with neither",
                markdown_sibling_error("//q:corpus_starlark", [], []),
                "",
            ),
        ],
    )
