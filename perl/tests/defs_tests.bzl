"""Wrapper conformance tests for Perl."""

load("//core:conformance.bzl", "dx_wrapper_contract_tests")
load("//core:defs.bzl", "expect_equal")
load("//core:entry.bzl", "dx_entry_quality_srcs")
load("//core:wrapper.bzl", "dx_language_kwargs")
load("//perl/private:defs.bzl", "PERL_DEFERRED_SKIP_TAGS", "PERL_QUALITY_TOOLS", "PERL_SOURCE_EXTS", "PERL_SOURCE_SPECS")
load("//perl/private:platforms.bzl", "PERL_PLATFORMS")

def perl_wrapper_contract_tests(name):
    """Declares the Perl wrapper contract tests."""
    dx_wrapper_contract_tests(
        name = name,
        size = "small",
        extra_checks = [
            expect_equal(
                "wrappers carry perlcritic and perltidy",
                sorted(PERL_QUALITY_TOOLS.keys()),
                [
                    "perlcritic",
                    "perltidy",
                ],
            ),
            expect_equal(
                "one perl class owns scripts, modules and tests",
                PERL_SOURCE_SPECS,
                [(
                    "perl",
                    [
                        "pl",
                        "pm",
                        "t",
                    ],
                )],
            ),
            expect_equal(
                "wrappers accept only Perl sources",
                PERL_SOURCE_EXTS,
                [
                    ".pl",
                    ".pm",
                    ".t",
                ],
            ),
            expect_equal(
                "main leads the owned sources",
                dx_entry_quality_srcs(
                    "main.pl",
                    ["Util.pm"],
                ),
                [
                    "main.pl",
                    "Util.pm",
                ],
            ),
            expect_equal(
                "main listed in srcs is owned once",
                dx_entry_quality_srcs(
                    "main.pl",
                    [
                        "main.pl",
                        "Util.pm",
                    ],
                ),
                [
                    "main.pl",
                    "Util.pm",
                ],
            ),
            expect_equal(
                "rules_perl platforms are the five it ships",
                sorted(PERL_PLATFORMS.keys()),
                [
                    "linux_arm64",
                    "linux_x86_64",
                    "macos_arm64",
                    "macos_x86_64",
                    "windows_x86_64",
                ],
            ),
            expect_equal(
                "no main and no srcs owns nothing",
                dx_entry_quality_srcs(None, None),
                [],
            ),
            expect_equal(
                "perl rules add no-typecheck once",
                dx_language_kwargs(
                    {"tags": [
                        "cpu:4",
                        "no-typecheck",
                    ]},
                    PERL_DEFERRED_SKIP_TAGS,
                )["tags"],
                [
                    "cpu:4",
                    "no-typecheck",
                ],
            ),
            expect_equal(
                "perl rules add no-typecheck to untagged targets",
                dx_language_kwargs({}, PERL_DEFERRED_SKIP_TAGS)["tags"],
                ["no-typecheck"],
            ),
        ],
        file_classes = ["perl"],
    )
