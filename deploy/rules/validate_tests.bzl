"""Unit tests for the shared deploy charset validators."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate.bzl", "EMBED_UNSAFE", "NAME_CHARS", "genrule_arg", "has_any", "only_chars")

def validate_unit_tests(name):
    """Declares unit tests for the shared deploy validators."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "only_chars accepts a value inside the charset",
                only_chars("v1.2.3-rc_1", NAME_CHARS),
                True,
            ),
            expect_equal(
                "only_chars rejects a char outside the charset",
                only_chars("v1;rm", NAME_CHARS),
                False,
            ),
            expect_equal(
                "only_chars accepts an empty value",
                only_chars("", NAME_CHARS),
                True,
            ),
            expect_equal(
                "has_any finds an embed-unsafe char",
                has_any("my file.tar", EMBED_UNSAFE),
                True,
            ),
            expect_equal(
                "has_any misses when no char matches",
                has_any("my_file.tar", EMBED_UNSAFE),
                False,
            ),
            expect_equal(
                "EMBED_UNSAFE keeps its 7 entries",
                len(EMBED_UNSAFE),
                7,
            ),
            expect_equal("genrule_arg quotes a plain value", genrule_arg("dx"), "'dx'"),
            expect_equal("genrule_arg escapes a single quote", genrule_arg("O'Neil"), "'O'\\''Neil'"),
            expect_equal("genrule_arg doubles dollars", genrule_arg("a$HOME"), "'a$$HOME'"),
            expect_equal("genrule_arg keeps backticks inside quotes", genrule_arg("`id`"), "'`id`'"),
            expect_equal("genrule_arg keeps double quotes inside quotes", genrule_arg("\"x\""), "'\"x\"'"),
            expect_equal("genrule_arg quotes the empty string", genrule_arg(""), "''"),
        ],
    )
