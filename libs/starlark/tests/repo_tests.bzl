"""Unit tests for the repository rule helpers."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:repo.bzl", "crlf_text")

def repo_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("mixed endings", crlf_text("a\nb\r\nc"), "a\r\nb\r\nc"),
            expect_equal("already crlf", crlf_text("x\r\n"), "x\r\n"),
            expect_equal("empty", crlf_text(""), ""),
        ],
    )
