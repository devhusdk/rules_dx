"""Wrapper conformance tests for Perl."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "PERL_QUALITY_TOOLS", "PERL_SOURCE_EXTS", "PERL_SOURCE_SPECS", "perl_quality_srcs")

def perl_wrapper_contract_tests(name):
    """Instantiates Perl wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["perl"],
        extra_checks = [
            expect_equal(
                "wrappers carry perltidy",
                sorted(PERL_QUALITY_TOOLS.keys()),
                ["perltidy"],
            ),
            expect_equal(
                "one perl class owns scripts, modules and tests",
                PERL_SOURCE_SPECS,
                [("perl", ["pl", "pm", "t"])],
            ),
            expect_equal(
                "wrappers accept only Perl sources",
                PERL_SOURCE_EXTS,
                [".pl", ".pm", ".t"],
            ),
            expect_equal(
                "main leads the owned sources",
                perl_quality_srcs("main.pl", ["Util.pm"]),
                ["main.pl", "Util.pm"],
            ),
            expect_equal(
                "main listed in srcs is owned once",
                perl_quality_srcs("main.pl", ["main.pl", "Util.pm"]),
                ["main.pl", "Util.pm"],
            ),
            expect_equal(
                "no main and no srcs owns nothing",
                perl_quality_srcs(None, None),
                [],
            ),
        ],
    )
