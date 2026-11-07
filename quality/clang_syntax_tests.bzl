"""Unit tests for the clang_syntax argv rewrite."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//cc/toolchains:bindings.bzl", "cc_compiler_identity")
load(":clang_syntax.bzl", "clang_action_env", "clang_syntax_argv", "clang_tidy_argv", "clang_syntax_source", "cuda_tidy_argv")

_CLANG = "external/llvm/bin/clang"

_LLVM = cc_compiler_identity("clang", "x86_64-unknown-linux-gnu")
_GCC = cc_compiler_identity("gcc", "x86_64-linux-gnu")
_CL = cc_compiler_identity("msvc-cl", "x86_64-pc-windows-msvc")
_CLANG_CL = cc_compiler_identity("clang-cl", "x86_64-pc-windows-msvc")
_UNKNOWN = cc_compiler_identity("", "")

def _rewrite(argv, src = "a.c", identity = _LLVM):
    return clang_syntax_argv(argv, src, identity)

def clang_syntax_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "CUDA: nvcc flags are translated to clang -x cuda with the toolkit root",
                cuda_tidy_argv("tidy", ["external/cuda/bin/nvcc", "-Iinc", "-DX=1", "-Xcompiler", "-fPIC,-isystem/x", "-gencode", "arch=compute_80,code=sm_80", "-rdc=true", "-std=c++17", "-c", "a.cu", "-o", "a.o"], "a.cu").argv,
                ["tidy", "a.cu", "--", "-x", "cuda", "--cuda-host-only", "-nocudalib", "-Wno-unknown-cuda-version", "--cuda-path=external/cuda", "-Iinc", "-DX=1", "-isystem/x", "-std=c++17"],
            ),
            expect_equal(
                "clang-tidy takes the config file right after the executable",
                clang_tidy_argv("tidy", [_CLANG, "-c", "a.c"], "a.c", _LLVM, "q/.clang-tidy").argv[:4],
                ["tidy", "--config-file=q/.clang-tidy", "a.c", "--"],
            ),
            expect_equal(
                "CUDA clang-tidy takes the config file right after the executable",
                cuda_tidy_argv("tidy", ["external/cuda/bin/nvcc", "-c", "a.cu"], "a.cu", "q/.clang-tidy").argv[:4],
                ["tidy", "--config-file=q/.clang-tidy", "a.cu", "--"],
            ),
            expect_equal(
                "clang-tidy gets the source then -- then the compile flags",
                clang_tidy_argv("tidy", [_CLANG, "-DX=1", "-c", "a.c", "-o", "a.o"], "a.c", _LLVM).argv,
                ["tidy", "a.c", "--", "-DX=1", "-fno-caret-diagnostics", "-fno-color-diagnostics"],
            ),
            expect_equal(
                "clang-tidy drops gcc-only flags under gcc",
                clang_tidy_argv("tidy", ["/usr/bin/gcc", "-fno-canonical-system-headers", "-DX=1", "-c", "a.c"], "a.c", _GCC).argv,
                ["tidy", "a.c", "--", "-DX=1", "-c"],
            ),
            expect_equal(
                "clang-tidy keeps Apple placeholders for the runner and Objective-C flags",
                clang_tidy_argv("tidy", ["wrapped_clang", "-isysroot", "__BAZEL_XCODE_SDKROOT__", "-fobjc-arc", "-c", "pkg/a.m"], "pkg/a.m", _LLVM).argv[:6],
                ["tidy", "pkg/a.m", "--", "-isysroot", "__BAZEL_XCODE_SDKROOT__", "-fobjc-arc"],
            ),
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
                _rewrite(["/usr/bin/gcc", "-c", "a.c", "-o", "a.o"], "a.c", _GCC).argv,
                ["/usr/bin/gcc", "-fsyntax-only", "-c", "a.c", "-fno-diagnostics-show-caret", "-fdiagnostics-color=never"],
            ),
            expect_equal(
                "a header with -x replays as given",
                _rewrite([_CLANG, "-x", "c++-header", "-c", "a.h"], "a.h").argv[1:4],
                ["-x", "c++-header", "-fsyntax-only"],
            ),
            expect_equal(
                "copts response files are rejected",
                _rewrite([_CLANG, "@a.params", "-c", "a.c"]).error,
                "response files in copts are not supported (found @a.params)",
            ),
            expect_equal(
                "cl.exe replays with /Zs, drops output flags and reports columns",
                _rewrite(["cl.exe", "/nologo", "/DX=1", "/c", "a.c", "/Foa.obj", "/Fdv.pdb", "-showIncludes", "/sourceDependencies", "a.json", "/I", "inc"], "a.c", _CL).argv,
                ["cl.exe", "/nologo", "/DX=1", "/Zs", "a.c", "/I", "inc", "/diagnostics:column"],
            ),
            expect_equal(
                "clang-cl replays with /Zs and keeps its flags",
                _rewrite(["clang-cl.exe", "-c", "a.c", "/Foa.obj", "/std:c++17", "-m64"], "a.c", _CLANG_CL).argv,
                ["clang-cl.exe", "/Zs", "a.c", "/std:c++17", "-m64", "/diagnostics:column"],
            ),
            expect_equal(
                "GNU drivers never get MSVC flags",
                "/diagnostics:column" in _rewrite([_CLANG, "-c", "a.c"]).argv,
                False,
            ),
            expect_equal(
                "an unknown compiler gets no diagnostic flags",
                _rewrite(["cc", "-c", "a.c"], "a.c", _UNKNOWN).argv,
                ["cc", "-fsyntax-only", "a.c"],
            ),
            expect_equal(
                "MSVC with two /c is rejected",
                _rewrite(["cl.exe", "/c", "/c", "a.c"], "a.c", _CL).error != "",
                True,
            ),
            expect_equal(
                "MSVC with a missing source is rejected",
                _rewrite(["cl.exe", "/c", "b.c"], "a.c", _CL).error != "",
                True,
            ),
            expect_equal(
                "MSVC copts response files are rejected",
                _rewrite(["cl.exe", "@a.rsp", "/c", "a.c"], "a.c", _CL).error,
                "response files in copts are not supported (found @a.rsp)",
            ),
            expect_equal(
                "clang-tidy projects cl.exe into clang-cl mode",
                clang_tidy_argv("tidy", ["cl.exe", "/nologo", "/DX=1", "/c", "a.c", "/Foa.obj", "/Fdv.pdb", "/showIncludes", "/sourceDependencies", "a.json", "/I", "inc"], "a.c", _CL).argv,
                ["tidy", "a.c", "--", "--driver-mode=cl", "--target=x86_64-pc-windows-msvc", "/nologo", "/DX=1", "/I", "inc"],
            ),
            expect_equal(
                "clang-tidy turns the action INCLUDE into /imsvc for MSVC",
                clang_tidy_argv("tidy", ["cl.exe", "/c", "a.c"], "a.c", _CL, "", {"INCLUDE": "C:\\crt\\include;C:\\sdk\\ucrt;", "EXTERNAL_INCLUDE": "D:\\ext"}).argv,
                ["tidy", "a.c", "--", "--driver-mode=cl", "--target=x86_64-pc-windows-msvc", "/imsvcC:/crt/include", "/imsvcC:/sdk/ucrt", "/imsvcD:/ext", "/X"],
            ),
            expect_equal(
                "clang-tidy keeps argv /imsvc over the action INCLUDE",
                clang_tidy_argv("tidy", ["cl.exe", "/imsvcx", "/c", "a.c"], "a.c", _CL, "", {"INCLUDE": "C:\\crt"}).argv,
                ["tidy", "a.c", "--", "--driver-mode=cl", "--target=x86_64-pc-windows-msvc", "/imsvcx"],
            ),
            expect_equal(
                "clang-tidy projects clang-cl and keeps its flags in order",
                clang_tidy_argv("tidy", ["clang-cl.exe", "-c", "a.c", "/Foa.obj", "/std:c++17", "-m64"], "a.c", _CLANG_CL, "q/.clang-tidy").argv,
                ["tidy", "--config-file=q/.clang-tidy", "a.c", "--", "--driver-mode=cl", "--target=x86_64-pc-windows-msvc", "/std:c++17", "-m64"],
            ),
            expect_equal(
                "MSVC clang actions turn off MSYS path conversion",
                clang_action_env({"A": "1"}, "msvc"),
                {"A": "1", "MSYS_NO_PATHCONV": "1", "MSYS2_ARG_CONV_EXCL": "*", "MSYS2_ENV_CONV_EXCL": "*"},
            ),
            expect_equal(
                "other clang actions keep their env",
                clang_action_env({"A": "1"}, "gcc"),
                {"A": "1"},
            ),
            expect_equal(
                "clang-tidy names precompiled headers for MSVC",
                clang_tidy_argv("tidy", ["cl.exe", "/Yuh.h", "/c", "a.c"], "a.c", _CL).error,
                "clang-tidy does not support precompiled headers (flag /Yuh.h)",
            ),
            expect_equal(
                "clang-tidy names a CUDA MSVC host compiler flag",
                cuda_tidy_argv("tidy", ["external/cuda/bin/nvcc", "-Xcompiler", "/MD", "-c", "a.cu"], "a.cu").error,
                "clang-tidy does not support nvcc with an MSVC host compiler (flag /MD)",
            ),
            expect_equal(
                "clang-tidy names a CUDA cl.exe host compiler",
                cuda_tidy_argv("tidy", ["external/cuda/bin/nvcc", "-ccbin", "C:\\vs\\cl.exe", "-c", "a.cu"], "a.cu").error,
                "clang-tidy does not support nvcc with an MSVC host compiler (flag -ccbin C:\\vs\\cl.exe)",
            ),
            expect_equal(
                "clang-tidy accepts nvcc.exe with backslash paths",
                cuda_tidy_argv("tidy", ["C:\\cuda\\bin\\nvcc.exe", "-c", "a.cu"], "a.cu").argv[-1],
                "--cuda-path=C:/cuda",
            ),
            expect_equal(
                "the source is the argument after /c",
                clang_syntax_source(["cl.exe", "/DX", "/c", "a.c", "/Foa.obj"]),
                "a.c",
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
                "inl and ipp headers are named as the source",
                [clang_syntax_source([_CLANG, "-xc++-header", "-fsyntax-only", "pkg/a." + e]) for e in ["inl", "ipp"]],
                ["pkg/a.inl", "pkg/a.ipp"],
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
