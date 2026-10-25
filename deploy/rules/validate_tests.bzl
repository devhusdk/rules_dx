"""Unit tests for the shared deploy charset validators."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate.bzl", "EMBED_UNSAFE", "LAUNCHER_UNSAFE", "NAME_CHARS", "VERSION_CHARS", "charset_schema_error", "has_any", "only_chars")

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
                "charset_schema_error accepts launcher-safe charsets",
                charset_schema_error(1, [
                    ("demo name", NAME_CHARS, LAUNCHER_UNSAFE + ["+"]),
                    ("demo version", VERSION_CHARS, LAUNCHER_UNSAFE),
                ]),
                "",
            ),
            expect_equal(
                "charset_schema_error rejects an unsupported schema version",
                charset_schema_error(2, [("demo name", NAME_CHARS, LAUNCHER_UNSAFE)]),
                "demo name: unsupported schema v2 (want v1)",
            ),
            expect_equal(
                "charset_schema_error rejects an empty charset",
                charset_schema_error(1, [("demo name", "", LAUNCHER_UNSAFE)]),
                "demo name: want a non-empty charset (schema v1)",
            ),
            expect_equal(
                "charset_schema_error rejects a non-string charset",
                charset_schema_error(1, [("demo name", None, LAUNCHER_UNSAFE)]),
                "demo name: want a non-empty charset (schema v1)",
            ),
            expect_equal(
                "charset_schema_error rejects a duplicate charset char",
                charset_schema_error(1, [("demo name", "abca", LAUNCHER_UNSAFE)]),
                "demo name: duplicate charset char 'a'",
            ),
            expect_equal(
                "charset_schema_error rejects an unsafe charset char",
                charset_schema_error(1, [("demo name", "ab+", LAUNCHER_UNSAFE + ["+"])]),
                "demo name: unsafe charset char '+' (must stay launcher-safe)",
            ),
            expect_equal(
                "charset_schema_error reports only the first failing charset",
                charset_schema_error(1, [
                    ("demo name", NAME_CHARS, LAUNCHER_UNSAFE),
                    ("demo version", "a/b", LAUNCHER_UNSAFE),
                    ("demo tag", "", LAUNCHER_UNSAFE),
                ]),
                "demo version: unsafe charset char '/' (must stay launcher-safe)",
            ),
            expect_equal(
                "LAUNCHER_UNSAFE keeps its 27 entries",
                len(LAUNCHER_UNSAFE),
                27,
            ),
            expect_equal(
                "EMBED_UNSAFE keeps its 7 entries",
                len(EMBED_UNSAFE),
                7,
            ),
        ],
    )
