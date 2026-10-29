"""Unit tests for the Fortran, D, OCaml and Zig compile replay rewrites."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "real_supported_classes")
load(":compile_check.bzl", "COMPILE_CHECK_TOOLS", "flang_check_argv", "ldc_check_argv", "ocaml_aliases", "ocaml_check_argv", "zig_check_argv", "zig_replayable")

_FLANG = "external/flang/bin/flang"
_LDC = "external/ldc/bin/ldmd2"
_ZIG = "external/zig/zig"
_OCAMLC = "external/ocaml/bin/ocamlc.opt"
_OCAMLOPT = "external/ocaml/bin/ocamlopt.opt"
_STAGED = "bazel-out/k8/bin/p/__obazl/M"

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
                "the replayed compile tools include flang, ldc, ocaml and zig",
                COMPILE_CHECK_TOOLS,
                ["flang", "ldc", "ocaml", "zig"],
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
                "ocaml replays a single implementation into scratch, stopping after typing",
                ocaml_check_argv([_OCAMLC, "-bin-annot", "-I", "in", "-c", "-impl", "p/m.ml", "-o", "out/M.cmo"], "tmp").steps,
                [[_OCAMLC, "-stop-after", "typing", "-error-style", "short", "-color", "never", "-I", "in", "-c", "-impl", "p/m.ml", "-o", "tmp/M.cmo"]],
            ),
            expect_equal(
                "ocaml replays a signature as -intf with a cmi in scratch",
                ocaml_check_argv([_OCAMLC, "-c", "-o", "out/M.cmi", "-intf", "p/m.mli"], "tmp").steps,
                [[_OCAMLC, "-stop-after", "typing", "-error-style", "short", "-color", "never", "-c", "-intf", "p/m.mli", "-o", "tmp/M.cmi"]],
            ),
            expect_equal(
                "ocaml splits the two-file form so nothing is written next to the sources",
                ocaml_check_argv([_OCAMLC, "-bin-annot", "-I", "in", "-I", "bazel-out/k8/bin/p/__obazl", "-c", _STAGED + ".mli", _STAGED + ".ml"], "tmp").steps,
                [
                    [_OCAMLC, "-stop-after", "typing", "-error-style", "short", "-color", "never", "-I", "in", "-I", "bazel-out/k8/bin/p/__obazl", "-c", "-intf", _STAGED + ".mli", "-o", "tmp/M.cmi"],
                    [_OCAMLC, "-stop-after", "typing", "-error-style", "short", "-color", "never", "-I", "tmp", "-I", "in", "-I", "bazel-out/k8/bin/p/__obazl", "-c", "-impl", _STAGED + ".ml", "-o", "tmp/M.cmo"],
                ],
            ),
            expect_equal(
                "ocaml native replays name a cmx",
                ocaml_check_argv([_OCAMLOPT, "-c", _STAGED + ".mli", _STAGED + ".ml"], "tmp").steps[1][-1],
                "tmp/M.cmx",
            ),
            expect_equal(
                "ocaml reports the implementation as the source",
                ocaml_check_argv([_OCAMLC, "-c", "-impl", "p/m.ml", "-o", "out/M.cmo"], "tmp").source,
                "p/m.ml",
            ),
            expect_equal(
                "ocaml keeps -cmi-file and -open",
                ocaml_check_argv([_OCAMLC, "-open", "Ns", "-cmi-file", "in/M.cmi", "-c", "-impl", "p/m.ml", "-o", "out/M.cmo"], "tmp").argv[7:11],
                ["-open", "Ns", "-cmi-file", "in/M.cmi"],
            ),
            expect_equal(
                "ocaml params files are rejected",
                ocaml_check_argv([_OCAMLC, "@a.params", "-c", "-impl", "p/m.ml", "-o", "out/M.cmo"], "tmp").error != "",
                True,
            ),
            expect_equal(
                "ocaml without -c is rejected",
                ocaml_check_argv([_OCAMLC, "-impl", "p/m.ml", "-o", "out/M.cmo"], "tmp").error != "",
                True,
            ),
            expect_equal(
                "ocaml without a source is rejected",
                ocaml_check_argv([_OCAMLC, "-c", "-o", "out/M.cmo"], "tmp").error != "",
                True,
            ),
            expect_equal(
                "ocaml sources naming different modules are rejected",
                ocaml_check_argv([_OCAMLC, "-c", "bazel-out/p/__obazl/A.mli", "bazel-out/p/__obazl/B.ml"], "tmp").error != "",
                True,
            ),
            expect_equal(
                "ocaml without scratch is rejected",
                ocaml_check_argv([_OCAMLC, "-c", "-impl", "p/m.ml", "-o", "out/M.cmo"], "").error != "",
                True,
            ),
            expect_equal(
                "ocaml aliases map staged sources back to the owned files",
                ocaml_aliases([_STAGED + ".mli", _STAGED + ".ml"], ["p/m.ml", "p/m.mli"]),
                [(_STAGED + ".mli", "p/m.mli"), (_STAGED + ".ml", "p/m.ml")],
            ),
            expect_equal(
                "ocaml owned sources need no alias",
                ocaml_aliases(["p/m.ml"], ["p/m.ml"]),
                [],
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
