"""Unit tests for the shared deploy charset validators."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":validate.bzl", "NAME_CHARS", "genrule_arg", "only_chars", "parse_size_budget", "size_budget_error")

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
            expect_equal("genrule_arg quotes a plain value", genrule_arg("dx"), "'dx'"),
            expect_equal("genrule_arg escapes a single quote", genrule_arg("O'Neil"), "'O'\\''Neil'"),
            expect_equal("genrule_arg doubles dollars", genrule_arg("a$HOME"), "'a$$HOME'"),
            expect_equal("genrule_arg keeps backticks inside quotes", genrule_arg("`id`"), "'`id`'"),
            expect_equal("genrule_arg keeps double quotes inside quotes", genrule_arg("\"x\""), "'\"x\"'"),
            expect_equal("genrule_arg quotes the empty string", genrule_arg(""), "''"),
            expect_equal("size_budget_error accepts no budget", size_budget_error(""), ""),
            expect_equal("size_budget_error accepts a megabyte budget", size_budget_error("25MB"), ""),
            expect_equal("size_budget_error accepts a plain byte budget", size_budget_error("512B"), ""),
            expect_equal(
                "size_budget_error rejects a missing unit",
                size_budget_error("25"),
                "size_budget '25': want a positive whole number and a unit, " +
                "for example '25MB' (B, KB, MB or GB, decimal)",
            ),
            expect_equal(
                "size_budget_error rejects zero",
                size_budget_error("0MB"),
                "size_budget '0MB': want a positive whole number and a unit, " +
                "for example '25MB' (B, KB, MB or GB, decimal)",
            ),
            expect_equal(
                "size_budget_error rejects a fraction",
                size_budget_error("1.5MB") != "",
                True,
            ),
            expect_equal("parse_size_budget reads megabytes as decimal", parse_size_budget("25MB"), 25000000),
            expect_equal("parse_size_budget reads gigabytes", parse_size_budget("2GB"), 2000000000),
            expect_equal("parse_size_budget reads kilobytes", parse_size_budget("3KB"), 3000),
            expect_equal("parse_size_budget reads bytes", parse_size_budget("7B"), 7),
            expect_equal("parse_size_budget gives 0 for no budget", parse_size_budget(""), 0),
        ],
    )
