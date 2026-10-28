"""Unit tests for the nvcc_syntax argv rewrite."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":nvcc_syntax.bzl", "nvcc_syntax_argv", "nvcc_syntax_select", "nvcc_syntax_source")

_NVCC = "external/cuda/bin/nvcc"

def _rewrite(argv, src = "a.cu"):
    return nvcc_syntax_argv(argv, src)

def nvcc_syntax_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "-c becomes -cuda and -o goes to /dev/null",
                _rewrite([_NVCC, "-DX=1", "-c", "a.cu", "-o", "bazel-out/a.o"]).argv,
                [_NVCC, "-DX=1", "-cuda", "a.cu", "-o", "/dev/null"],
            ),
            expect_equal(
                "--output-file= goes to /dev/null",
                _rewrite([_NVCC, "-c", "a.cu", "--output-file=a.o"]).argv,
                [_NVCC, "-cuda", "a.cu", "--output-file=/dev/null"],
            ),
            expect_equal(
                "gencode, Xcompiler and ccbin stay byte-identical",
                _rewrite([_NVCC, "-gencode", "arch=compute_80,code=sm_80", "-Xcompiler", "-fPIC,-O2", "-ccbin", "/usr/bin/clang", "-rdc=true", "-c", "a.cu", "-o", "a.o"]).argv,
                [_NVCC, "-gencode", "arch=compute_80,code=sm_80", "-Xcompiler", "-fPIC,-O2", "-ccbin", "/usr/bin/clang", "-rdc=true", "-cuda", "a.cu", "-o", "/dev/null"],
            ),
            expect_equal(
                "dependency and keep flags are dropped",
                _rewrite([_NVCC, "-MD", "-MMD", "-MF", "a.d", "-MT", "a.o", "--generate-dependencies-with-compile", "--dependency-output", "a.d", "-keep", "-keep-dir", "k", "-save-temps", "-c", "a.cu", "-o", "a.o"]).argv,
                [_NVCC, "-cuda", "a.cu", "-o", "/dev/null"],
            ),
            expect_equal(
                "-fdevice-syntax-only is never added",
                "-fdevice-syntax-only" in _rewrite([_NVCC, "-c", "a.cu", "-o", "a.o"]).argv,
                False,
            ),
            expect_equal(
                "a compiler other than nvcc is an error",
                _rewrite(["external/llvm/bin/clang", "-c", "a.cu"]).error != "",
                True,
            ),
            expect_equal(
                "an empty command line is an error",
                _rewrite([]).error != "",
                True,
            ),
            expect_equal(
                "a params file is an error",
                _rewrite([_NVCC, "@a.params", "-c", "a.cu"]).error != "",
                True,
            ),
            expect_equal(
                "another phase flag is an error",
                _rewrite([_NVCC, "-dc", "a.cu"]).error != "",
                True,
            ),
            expect_equal(
                "zero -c is an error",
                _rewrite([_NVCC, "a.cu"]).error != "",
                True,
            ),
            expect_equal(
                "two -c is an error",
                _rewrite([_NVCC, "-c", "a.cu", "-c", "b.cu"]).error != "",
                True,
            ),
            expect_equal(
                "the source must stay on the command line",
                _rewrite([_NVCC, "-c", "b.cu"]).error != "",
                True,
            ),
            expect_equal(
                "the source is the argument after -c",
                nvcc_syntax_source([_NVCC, "-DX", "-c", "a.cu", "-o", "a.o"]),
                "a.cu",
            ),
            expect_equal(
                "no -c means no source",
                nvcc_syntax_source([_NVCC, "a.cu"]),
                "",
            ),
            expect_equal(
                "one replay per source, preferring the non-pic variant",
                nvcc_syntax_select([
                    [_NVCC, "-Xcompiler", "-fPIC", "-c", "a.cu", "-o", "a.pic.o"],
                    [_NVCC, "-c", "a.cu", "-o", "a.o"],
                    [_NVCC, "-Xcompiler", "-fPIC", "-c", "b.cu", "-o", "b.pic.o"],
                    [_NVCC, "-Xcompiler", "-fPIC", "-rdc=true", "-c", "b.cu", "-o", "b.rdc.o"],
                ]),
                [1, 2],
            ),
        ],
    )
