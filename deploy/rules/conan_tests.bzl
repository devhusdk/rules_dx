"""Unit tests for the Conan publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_conan.bzl", "conan_name_error", "conan_remote_error", "conan_remote_url_error", "conan_version_error")

def conan_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("conan_name_error accepts lowercase names", conan_name_error("conan_demo"), ""),
            expect_equal(
                "conan_name_error rejects uppercase",
                conan_name_error("Conan"),
                "conan_deploy: invalid package name 'Conan': want only [a-z0-9_.-]",
            ),
            expect_equal("conan_version_error accepts dotted versions", conan_version_error("0.1.0"), ""),
            expect_equal(
                "conan_version_error rejects an empty version",
                conan_version_error(""),
                "conan_deploy: invalid version '': want a non-empty version (for example '0.1.0')",
            ),
            expect_equal("conan_remote_error accepts a name", conan_remote_error("center2"), ""),
            expect_equal(
                "conan_remote_error rejects separators",
                conan_remote_error("a/b"),
                "conan_deploy: invalid remote 'a/b': want only [a-z0-9_-]",
            ),
            expect_equal("conan_remote_url_error accepts https", conan_remote_url_error("https://center2.conan.io"), ""),
            expect_equal(
                "conan_remote_url_error rejects http",
                conan_remote_url_error("http://x"),
                "conan_deploy: invalid remote_url 'http://x': want an https:// URL",
            ),
        ],
    )
