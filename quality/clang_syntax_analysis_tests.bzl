"""Analysis tests for the clang syntax aspect."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")

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
    config_settings = {"//command_line_option:platforms": str(Label("//msix/platforms:windows_x86_64"))},
    extra_target_under_test_aspects = [clang_syntax_aspect],
)

def clang_syntax_analysis_tests(name):
    """Instantiates the clang syntax analysis tests as one suite."""
    _windows_argv_test(
        name = name + "_windows",
        size = "small",
        target_under_test = "//quality/testdata/clang_syntax:windows_param_file",
    )
    native.test_suite(name = name, tests = [name + "_windows"])
