"""Unit tests for oci_image_from_binary."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":image.bzl", "image_binary_name", "image_options_error")

def image_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "image_binary_name reads the target name",
                image_binary_name("//app:server"),
                "server",
            ),
            expect_equal(
                "image_binary_name reads a bare package label",
                image_binary_name("//app/server"),
                "server",
            ),
            expect_equal(
                "image_binary_name reads a local label",
                image_binary_name(":server"),
                "server",
            ),
            expect_equal(
                "image_options_error accepts the defaults",
                image_options_error(":server", "static", ["linux/amd64"]),
                "",
            ),
            expect_equal(
                "image_options_error accepts two platforms",
                image_options_error(":server", "cc", ["linux/amd64", "linux/arm64"]),
                "",
            ),
            expect_equal(
                "image_options_error rejects an unknown base",
                image_options_error(":server", "alpine", ["linux/amd64"]),
                "oci_image_from_binary: invalid base 'alpine': want one of cc, static",
            ),
            expect_equal(
                "image_options_error rejects an empty platform list",
                image_options_error(":server", "static", []),
                "oci_image_from_binary: want at least one platform, one of linux/amd64, linux/arm64",
            ),
            expect_equal(
                "image_options_error rejects an unknown platform",
                image_options_error(":server", "static", ["windows/amd64"]),
                "oci_image_from_binary: invalid platform 'windows/amd64': want one of linux/amd64, linux/arm64",
            ),
            expect_equal(
                "image_options_error rejects a repeated platform",
                image_options_error(":server", "static", ["linux/amd64", "linux/amd64"]),
                "oci_image_from_binary: want each platform once, got linux/amd64, linux/amd64",
            ),
        ],
    )
