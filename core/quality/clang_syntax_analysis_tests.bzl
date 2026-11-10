"""Analysis tests for the clang syntax aspect."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cc/private:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")
load("//core/quality/testdata:all_languages.bzl", "real_cc_lint_aspect")
load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")

_syntax_subject = config_subject_rule([clang_syntax_aspect], [ClangSyntaxInfo])
_lint_subject = config_subject_rule([real_cc_lint_aspect])

def _windows_argv_impl(ctx):
    """Checks a Windows target gets a full command line even with compiler_param_file on."""
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ClangSyntaxInfo]
    asserts.true(env, len(info.argv) > 0, "want one replay")
    for argv in info.argv:
        for arg in argv:
            asserts.false(env, arg.startswith("@"), "response file in " + str(argv))
        asserts.true(env, "clean.c" in " ".join(argv), "source missing in " + str(argv))
    for source in info.sources:
        asserts.true(env, source != "", "empty source")
    return analysistest.end(env)

_windows_argv_test = analysistest.make(
    _windows_argv_impl,
    config_settings = {"//command_line_option:platforms": str(Label("@llvm//platforms:windows_x86_64"))},
    extra_target_under_test_aspects = [clang_syntax_aspect],
)

def _no_tidy_config_impl(ctx):
    """Checks a policy without a clang_tidy config still replays the syntax check, without clang-tidy."""
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ClangSyntaxInfo]
    asserts.true(env, len(info.diagnostics) > 0, "want one syntax replay")
    asserts.equals(env, [], info.tidy_diagnostics)
    asserts.true(env, "has no clang_tidy config" in info.tidy_error, info.tidy_error)
    return analysistest.end(env)

_no_tidy_config_test = analysistest.make(
    _no_tidy_config_impl,
)

def _tidy_without_config_impl(ctx):
    """Checks selecting clang_tidy without a config fails with the missing-config message."""
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "has no clang_tidy config")
    return analysistest.end(env)

_tidy_without_config_test = analysistest.make(
    _tidy_without_config_impl,
    expect_failure = True,
)

def clang_syntax_analysis_tests(name):
    """Instantiates the clang syntax analysis tests as one suite."""
    _windows_argv_test(
        name = name + "_windows",
        size = "small",
        target_under_test = "//core/quality/testdata/clang_syntax:windows_param_file",
    )
    _syntax_subject(
        name = name + "_no_tidy_config_subject",
        config = "cc_syntax",
        tags = ["manual"],
        target = "//core/quality/testdata/clang_syntax:dirty",
    )
    _no_tidy_config_test(
        name = name + "_no_tidy_config",
        size = "small",
        target_under_test = name + "_no_tidy_config_subject",
    )
    _lint_subject(
        name = name + "_tidy_without_config_subject",
        config = "cc_tidy",
        tags = ["manual"],
        target = "//core/quality/testdata/clang_syntax:dirty",
    )
    _tidy_without_config_test(
        name = name + "_tidy_without_config",
        size = "small",
        target_under_test = name + "_tidy_without_config_subject",
    )
    native.test_suite(name = name, tests = [name + "_windows", name + "_no_tidy_config", name + "_tidy_without_config"])
