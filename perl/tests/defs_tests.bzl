"""Wrapper conformance tests for Perl."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:conformance.bzl", "coverage_env_tests", "dx_wrapper_contract_tests")
load("//core:defs.bzl", "expect_equal")
load("//core:entry.bzl", "dx_entry_quality_srcs")
load("//core:wrapper.bzl", "dx_language_kwargs")
load("//perl/private:defs.bzl", "PERL_DEFERRED_SKIP_TAGS", "PERL_QUALITY_TOOLS", "PERL_SOURCE_EXTS", "PERL_SOURCE_SPECS")
load("//perl/private:platforms.bzl", "PERL_PLATFORMS")

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    collected = [p for p in runfiles if p.endswith("perl/private/coverage/DxLcov.pm")]
    if ctx.attr.coverage:
        asserts.equals(env, "-MDxLcov", run_env.get("PERL5OPT"))
        asserts.true(env, run_env.get("PERL5LIB", "").endswith("perl/private/coverage"), "PERL5LIB lacks the collector: " + run_env.get("PERL5LIB", ""))
        asserts.true(env, collected != [], "runfiles lack DxLcov.pm")
        asserts.true(env, InstrumentedFilesInfo in target, "no InstrumentedFilesInfo")
    else:
        asserts.equals(env, None, run_env.get("PERL5OPT"))
        asserts.equals(env, None, run_env.get("PERL5LIB"))
        asserts.equals(env, [], collected)
    return analysistest.end(env)

_COVERAGE = coverage_env_tests(_coverage_env_impl)
_coverage_env_on_test = _COVERAGE.on
_coverage_env_off_test = _COVERAGE.off

def perl_coverage_env_test(name, target):
    """Asserts a Perl test loads the DxLcov collector under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)

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
                "perl platforms are the lang.json platforms",
                sorted(PERL_PLATFORMS.keys()),
                [
                    "linux_arm64",
                    "linux_x86_64",
                    "macos_arm64",
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
