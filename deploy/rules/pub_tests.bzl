"""Unit tests for the pub.dev publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":pub.bzl", "PUB_DEFAULT_HOSTED_URL", "pub_hosted_url_error", "pub_name_error", "pub_version_error")

def pub_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "pub_name_error accepts the demo package name",
                pub_name_error("pub_demo"),
                "",
            ),
            expect_equal(
                "pub_name_error rejects an empty name",
                pub_name_error(""),
                "pub_deploy: invalid package name '': want a non-empty " +
                "name (for example 'pub_demo')",
            ),
            expect_equal(
                "pub_name_error rejects dashes and capitals",
                pub_name_error("Pub-Demo"),
                "pub_deploy: invalid package name 'Pub-Demo': want only [a-z0-9_]",
            ),
            expect_equal(
                "pub_version_error accepts prerelease versions",
                pub_version_error("1.2.3-beta.1"),
                "",
            ),
            expect_equal(
                "pub_version_error rejects version characters",
                pub_version_error("1.0;curl evil"),
                "pub_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
            expect_equal(
                "pub_hosted_url_error accepts the default server",
                pub_hosted_url_error(PUB_DEFAULT_HOSTED_URL),
                "",
            ),
            expect_equal(
                "pub_hosted_url_error rejects plaintext URLs",
                pub_hosted_url_error("http://pub.example.com"),
                "pub_deploy: invalid hosted_url 'http://pub.example.com': " +
                "want an https URL so publishes never go over plaintext",
            ),
        ],
    )
