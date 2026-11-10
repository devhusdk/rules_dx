"""Unit tests for the opam publisher."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":validate_opam.bzl", "opam_file_error", "opam_name_error", "opam_version_error")

def opam_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("opam_name_error accepts a name", opam_name_error("demo-lib_2+x"), ""),
            expect_equal(
                "opam_name_error rejects a slash",
                opam_name_error("a/b"),
                "opam_deploy: invalid package 'a/b': want only [A-Za-z0-9_+-]",
            ),
            expect_equal("opam_version_error accepts a version", opam_version_error("1.2.0~rc1+git"), ""),
            expect_equal(
                "opam_version_error rejects empty",
                opam_version_error(""),
                "opam_deploy: invalid version '': want a non-empty version (for example '1.2.0')",
            ),
            expect_equal("opam_file_error accepts opam", opam_file_error("opam"), ""),
            expect_equal(
                "opam_file_error rejects other names",
                opam_file_error("demo.opam"),
                "opam_deploy: invalid opam_file 'demo.opam': want a file named opam",
            ),
        ],
    )
