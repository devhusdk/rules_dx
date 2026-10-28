"""Unit tests for the clang-tidy command line projection."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":clang_syntax.bzl", "clang_syntax_argv")
load(":clang_tidy.bzl", "clang_tidy_argv", "clang_tidy_indices")

_CLANG = "external/llvm/bin/clang"
_WRAPPED = "external/apple_support/crosstool/wrapped_clang"

def clang_tidy_unit_tests(name):
    objc = clang_syntax_argv(
        [_WRAPPED, "-DX=1", "-isysroot", "__BAZEL_XCODE_SDKROOT__", "-fobjc-arc", "-c", "pkg/a.m", "-o", "a.o"],
        "pkg/a.m",
    ).argv
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "generated and unowned translation units get no action",
                clang_tidy_indices(["pkg/a.m", "pkg/b.cc", "bazel-out/k8-fastbuild/bin/pkg/gen.c"], ["pkg/b.cc"]),
                [1],
            ),
            expect_equal(
                "without an owner list every translation unit is kept",
                clang_tidy_indices(["pkg/a.m", "pkg/b.cc"], None),
                [0, 1],
            ),
            expect_equal(
                "the compiler goes away and the source becomes the positional argument",
                clang_tidy_argv([_CLANG, "-DX=1", "-fsyntax-only", "a.c"], "a.c").argv,
                ["--quiet", "--checks=clang-diagnostic-*,clang-analyzer-*", "a.c", "--", "-DX=1", "-fsyntax-only"],
            ),
            expect_equal(
                "a config file is passed before the source",
                clang_tidy_argv([_CLANG, "-fsyntax-only", "a.c"], "a.c", "cfg/.clang-tidy").argv,
                ["--quiet", "--config-file=cfg/.clang-tidy", "a.c", "--", "-fsyntax-only"],
            ),
            expect_equal(
                "an Objective-C compile keeps its flags",
                clang_tidy_argv(objc, "pkg/a.m").argv,
                ["--quiet", "--checks=clang-diagnostic-*,clang-analyzer-*", "pkg/a.m", "--", "-DX=1", "-isysroot", "__BAZEL_XCODE_SDKROOT__", "-fobjc-arc", "-fsyntax-only", "-fno-caret-diagnostics", "-fno-color-diagnostics"],
            ),
            expect_equal(
                "only the last occurrence of the source is removed",
                clang_tidy_argv([_CLANG, "-include", "a.h", "-fsyntax-only", "a.h"], "a.h").argv,
                ["--quiet", "--checks=clang-diagnostic-*,clang-analyzer-*", "a.h", "--", "-include", "a.h", "-fsyntax-only"],
            ),
            expect_equal(
                "a gcc command line is rejected",
                clang_tidy_argv(["/usr/bin/gcc", "-fsyntax-only", "a.c"], "a.c").error != "",
                True,
            ),
            expect_equal(
                "a command line without the source is rejected",
                clang_tidy_argv([_CLANG, "-fsyntax-only", "b.c"], "a.c").error != "",
                True,
            ),
            expect_equal(
                "an empty command line is rejected",
                clang_tidy_argv([], "a.c").error != "",
                True,
            ),
        ],
    )
