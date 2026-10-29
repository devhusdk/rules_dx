"""Unit tests for the Fortran, D and Zig compile replay rewrites."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "real_supported_classes")
load(":compile_check.bzl", "COMPILE_CHECK_TOOLS", "flang_check_argv", "ldc_check_argv", "zig_check_argv", "zig_replayable")

_FLANG = "external/flang/bin/flang"
_LDC = "external/ldc/bin/ldmd2"
_ZIG = "external/zig/zig"

def compile_check_unit_tests(name):
    """Declares the rewrite unit tests."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "every replayed compile tool is a registry tool with typecheck support",
                [(tool, len(real_supported_classes(tool, "typecheck")) > 0) for tool in COMPILE_CHECK_TOOLS],
                [(tool, True) for tool in COMPILE_CHECK_TOOLS],
            ),
            expect_equal(
                "the replayed compile tools include flang, ldc and zig",
                COMPILE_CHECK_TOOLS,
                ["flang", "ldc", "zig"],
            ),
            expect_equal(
                "flang -c becomes -fsyntax-only, -o is dropped and modules go to scratch",
                flang_check_argv([_FLANG, "-ffree-form", "-Jout/mods", "-Iin/mods", "-c", "a.f90", "-o", "a.o"], "tmp").argv,
                [_FLANG, "-ffree-form", "-Jtmp", "-Iin/mods", "-fsyntax-only", "a.f90"],
            ),
            expect_equal(
                "flang reports the source",
                flang_check_argv([_FLANG, "-c", "a.f90", "-o", "a.o"], "tmp").source,
                "a.f90",
            ),
            expect_equal(
                "flang without a module flag still writes modules to scratch",
                flang_check_argv([_FLANG, "-c", "a.f90"], "tmp").argv,
                [_FLANG, "-fsyntax-only", "a.f90", "-Jtmp"],
            ),
            expect_equal(
                "flang without -c is rejected",
                flang_check_argv([_FLANG, "a.f90"], "tmp").error != "",
                True,
            ),
            expect_equal(
                "flang params files are rejected",
                flang_check_argv([_FLANG, "@a.params", "-c", "a.f90"], "tmp").error != "",
                True,
            ),
            expect_equal(
                "ldc drops -c and -of= and adds -o- with gnu errors",
                ldc_check_argv([_LDC, "-g", "a.d", "-I=src", "-c", "-of=out/a.o"], "").argv,
                [_LDC, "-g", "a.d", "-I=src", "-o-", "-verror-style=gnu", "-vcolumns"],
            ),
            expect_equal(
                "ldc drops -lib and keeps unit test flags",
                ldc_check_argv([_LDC, "-unittest", "-main", "a.d", "-lib", "-of=out/a.a"], "").argv,
                [_LDC, "-unittest", "-main", "a.d", "-o-", "-verror-style=gnu", "-vcolumns"],
            ),
            expect_equal(
                "ldc keeps -debug and -defaultlib flags",
                ldc_check_argv([_LDC, "-debug=x", "-defaultlib=phobos2", "a.d"], "").argv[1:3],
                ["-debug=x", "-defaultlib=phobos2"],
            ),
            expect_equal(
                "ldc without a D source is rejected",
                ldc_check_argv([_LDC, "-c"], "").error != "",
                True,
            ),
            expect_equal(
                "ldc params files are rejected",
                ldc_check_argv([_LDC, "@a.params", "a.d"], "").error != "",
                True,
            ),
            expect_equal(
                "zig drops every -femit flag and adds -fno-emit-bin",
                zig_check_argv([_ZIG, "build-exe", "-femit-bin=out/a", "-femit-asm=out/a.s", "-O", "Debug", "-Mroot=src/a.zig"], "").argv,
                [_ZIG, "build-exe", "-O", "Debug", "-Mroot=src/a.zig", "-fno-emit-bin"],
            ),
            expect_equal(
                "zig keeps an existing -fno-emit-bin once",
                zig_check_argv([_ZIG, "test", "--test-no-exec", "-fno-emit-bin", "-femit-llvm-bc=out/a.bc", "-Mroot=src/a.zig"], "").argv,
                [_ZIG, "test", "--test-no-exec", "-fno-emit-bin", "-Mroot=src/a.zig"],
            ),
            expect_equal(
                "zig reports the root module source",
                zig_check_argv([_ZIG, "build-lib", "--dep", "x=y", "-Mroot=src/a.zig", "-Mx=lib/x.zig"], "").source,
                "src/a.zig",
            ),
            expect_equal(
                "zig without a source is rejected",
                zig_check_argv([_ZIG, "build-exe", "-femit-bin=a"], "").error != "",
                True,
            ),
            expect_equal(
                "zig cc_common relink argv (no .zig root module) is rejected by the rewrite",
                zig_check_argv([_ZIG, "build-lib", "-fPIC", "-fcompiler-rt", "-femit-bin=out/a.a", "out/a.bc"], "").error != "",
                True,
            ),
            expect_equal(
                "zig relink argv is not replayable",
                zig_replayable([_ZIG, "build-lib", "-fPIC", "-fcompiler-rt", "-femit-bin=out/a.a", "out/a.bc"]),
                False,
            ),
            expect_equal(
                "zig root module argv is replayable",
                zig_replayable([_ZIG, "test", "-Mroot=src/a.zig"]),
                True,
            ),
            expect_equal(
                "zig params files are rejected",
                zig_check_argv([_ZIG, "build-exe", "@a.params"], "").error != "",
                True,
            ),
        ],
    )
