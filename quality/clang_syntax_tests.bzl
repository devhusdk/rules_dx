"""Unit tests for the clang_syntax argv rewrite."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":clang_syntax.bzl", "clang_syntax_argv", "clang_syntax_source")

_CLANG = "external/llvm/bin/clang"

def _rewrite(argv, src = "a.c"):
    return clang_syntax_argv(argv, src)

def clang_syntax_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "-c becomes -fsyntax-only and output flags are dropped",
                _rewrite([_CLANG, "-MD", "-MF", "a.d", "-DX=1", "-c", "a.c", "-o", "a.o"]).argv,
                [_CLANG, "-DX=1", "-fsyntax-only", "a.c", "-fno-caret-diagnostics", "-fno-color-diagnostics"],
            ),
            expect_equal(
                "serialized diagnostics and color are dropped",
                _rewrite([_CLANG, "--serialize-diagnostics", "a.dia", "-fcolor-diagnostics", "-c", "a.c"]).argv,
                [_CLANG, "-fsyntax-only", "a.c", "-fno-caret-diagnostics", "-fno-color-diagnostics"],
            ),
            expect_equal(
                "arguments with spaces survive untouched",
                _rewrite([_CLANG, "-DMSG=hello world", "-c", "a.c"]).argv[1],
                "-DMSG=hello world",
            ),
            expect_equal(
                "gcc gets the gcc diagnostic flags",
                _rewrite(["/usr/bin/gcc", "-c", "a.c", "-o", "a.o"]).argv,
                ["/usr/bin/gcc", "-fsyntax-only", "a.c", "-fno-diagnostics-show-caret", "-fdiagnostics-color=never"],
            ),
            expect_equal(
                "a header with -x replays as given",
                _rewrite([_CLANG, "-x", "c++-header", "-c", "a.h"], "a.h").argv[1:4],
                ["-x", "c++-header", "-fsyntax-only"],
            ),
            expect_equal(
                "params files are rejected",
                _rewrite([_CLANG, "@a.params", "-c", "a.c"]).error != "",
                True,
            ),
            expect_equal(
                "MSVC command lines are rejected",
                _rewrite(["cl.exe", "/c", "a.c", "/Foa.obj"]).error != "",
                True,
            ),
            expect_equal(
                "a command line without -c is rejected",
                _rewrite([_CLANG, "a.c"]).error != "",
                True,
            ),
            expect_equal(
                "a command line that omits its source is rejected",
                _rewrite([_CLANG, "-c", "b.c"]).error != "",
                True,
            ),
            expect_equal(
                "an empty command line is rejected",
                _rewrite([]).error != "",
                True,
            ),
            expect_equal(
                "the source is the argument after -c",
                clang_syntax_source([_CLANG, "-DX", "-c", "a.c", "-o", "a.o"]),
                "a.c",
            ),
            expect_equal(
                "a header-parsing action names its header as the source",
                clang_syntax_source([_CLANG, "-DX", "-iquote", ".", "-xc++-header", "-fsyntax-only", "pkg/a.h"]),
                "pkg/a.h",
            ),
            expect_equal(
                "a header-parsing action is replayed without a -c",
                _rewrite([_CLANG, "-iquote", ".", "-xc++-header", "-fsyntax-only", "pkg/a.h"], "pkg/a.h").error,
                "",
            ),
            expect_equal(
                "no -c names no source",
                clang_syntax_source([_CLANG, "a.c"]),
                "",
            ),
        ],
    )
