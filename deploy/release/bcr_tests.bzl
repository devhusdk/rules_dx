"""Unit tests for BCR submission tooling."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":bcr.bzl", "bcr_source_error")

def bcr_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "bcr source accepts rules_dx at 0.0.0 for shape checks",
                bcr_source_error("rules_dx", "0.0.0"),
                "",
            ),
            expect_equal(
                "bcr source accepts rules_dx at SemVer",
                bcr_source_error("rules_dx", "1.2.3"),
                "",
            ),
            expect_equal(
                "bcr source rejects a foreign module",
                bcr_source_error("other_rules", "1.2.3"),
                "bcr: invalid module 'other_rules': want 'rules_dx'",
            ),
            expect_equal(
                "bcr source rejects a non-SemVer version",
                bcr_source_error("rules_dx", "v1"),
                "bcr: invalid version 'v1': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')",
            ),
        ],
    )
