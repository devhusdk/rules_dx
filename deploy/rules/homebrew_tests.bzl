"""Unit tests for the Homebrew tap publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_homebrew.bzl", "homebrew_formula_error", "homebrew_repo_error")

def homebrew_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "homebrew_formula_error accepts a hyphenated name",
                homebrew_formula_error("my-tool"),
                "",
            ),
            expect_equal(
                "homebrew_formula_error rejects upper case",
                homebrew_formula_error("MyTool"),
                "homebrew_tap_deploy: invalid formula 'MyTool': want only [a-z0-9_-]",
            ),
            expect_equal(
                "homebrew_repo_error accepts owner/repository",
                homebrew_repo_error("tap", "org/homebrew-tap"),
                "",
            ),
            expect_equal(
                "homebrew_repo_error rejects a bare name",
                homebrew_repo_error("tap", "tap"),
                "homebrew_tap_deploy: invalid tap 'tap': want owner/repository " +
                "(for example 'org/homebrew-tap')",
            ),
        ],
    )
