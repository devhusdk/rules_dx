"""Unit tests for the unused-code roots helpers."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":unused_roots.bzl", "unused_is_public", "unused_main_files")

def unused_roots_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("public visibility is public", unused_is_public(["//visibility:public"]), True),
            expect_equal("a label with a repo prefix is public", unused_is_public(["@@//visibility:public"]), True),
            expect_equal("package groups are not public", unused_is_public(["//pkg:__subpackages__"]), False),
            expect_equal("no visibility is not public", unused_is_public(None), False),
            expect_equal("a binary without main uses its srcs", unused_main_files("py_binary", [], ["a.py", "b.py"]), ["a.py", "b.py"]),
            expect_equal("a test without main uses its srcs", unused_main_files("py_test", [], ["t.py"]), ["t.py"]),
            expect_equal("main wins over srcs", unused_main_files("py_binary", ["m.py"], ["a.py", "m.py"]), ["m.py"]),
            expect_equal("a library has no mains", unused_main_files("py_library", [], ["a.py"]), []),
        ],
    )
