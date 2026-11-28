"""Analysis tests for the clang syntax aspect."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//cc/private:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")
load("//core:failure_test.bzl", "failure_test")
load("//core/quality/testdata:all_languages.bzl", "real_cc_lint_aspect")
load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")

visibility("private")

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

def _sourceless_compile_impl(ctx):
    """Registers a compile action while holding no source file."""
    tool = ctx.actions.declare_file(ctx.label.name + ".tool")
    ctx.actions.write(tool, "", is_executable = True)
    obj = ctx.actions.declare_file(ctx.label.name + ".o")
    ctx.actions.run(executable = tool, outputs = [obj], arguments = ["/MD"], mnemonic = "CppCompile")
    return [CcInfo(), DefaultInfo(files = depset([obj]))]

_sourceless_compile = rule(
    implementation = _sourceless_compile_impl,
    doc = "A C/C++ target whose compile action names no source.",
)

def _sourceless_impl(ctx):
    """Checks a target that holds no workspace source gets no replay."""
    env = analysistest.begin(ctx)
    asserts.false(env, ClangSyntaxInfo in analysistest.target_under_test(env), "want no replay")
    return analysistest.end(env)

_sourceless_test = analysistest.make(
    _sourceless_impl,
    extra_target_under_test_aspects = [clang_syntax_aspect],
)

def _header_only_impl(ctx):
    """Checks a header-only library parses and clang-tidies each header, a .h one as a C header."""
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ClangSyntaxInfo]
    asserts.equals(env, ["core/quality/testdata/clang_syntax/dep.h"], info.sources)
    asserts.true(env, "-xc-header" in info.argv[0], "want a C header parse in " + str(info.argv[0]))
    asserts.equals(env, "", info.tidy_error)
    asserts.equals(env, 1, len(info.tidy_diagnostics))
    return analysistest.end(env)

_header_only_test = analysistest.make(
    _header_only_impl,
    extra_target_under_test_aspects = [clang_syntax_aspect],
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
    failure_test(
        name = name + "_tidy_without_config",
        size = "small",
        target = name + "_tidy_without_config_subject",
        expected_failure_substring = "has no clang_tidy config",
    )
    _sourceless_compile(
        name = name + "_sourceless_target",
        tags = ["manual"],
    )
    _sourceless_test(
        name = name + "_sourceless",
        size = "small",
        target_under_test = name + "_sourceless_target",
    )
    _header_only_test(
        name = name + "_header_only",
        size = "small",
        target_under_test = "//core/quality/testdata/clang_syntax:header_only",
    )
    native.test_suite(name = name, tests = [name + "_windows", name + "_no_tidy_config", name + "_tidy_without_config", name + "_sourceless", name + "_header_only"])
