"""Unit tests for the Packagist publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_packagist.bzl", "packagist_composer_error", "packagist_name_error", "packagist_repository_error")

def packagist_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("packagist_name_error accepts vendor/name", packagist_name_error("acme/demo-lib"), ""),
            expect_equal(
                "packagist_name_error rejects a bare name",
                packagist_name_error("demo"),
                "packagist_deploy: invalid package 'demo': want vendor/name (for example 'acme/demo')",
            ),
            expect_equal(
                "packagist_name_error rejects uppercase",
                packagist_name_error("Acme/demo"),
                "packagist_deploy: invalid package 'Acme/demo': want only [a-z0-9_.-] in each part",
            ),
            expect_equal("packagist_repository_error accepts https", packagist_repository_error("https://github.com/acme/demo"), ""),
            expect_equal(
                "packagist_repository_error rejects http",
                packagist_repository_error("http://github.com/acme/demo"),
                "packagist_deploy: invalid repository_url 'http://github.com/acme/demo': want an https:// URL",
            ),
            expect_equal(
                "packagist_repository_error rejects shell characters",
                packagist_repository_error("https://x/a;b"),
                "packagist_deploy: invalid repository_url 'https://x/a;b': want only [A-Za-z0-9:/._~-]",
            ),
            expect_equal("packagist_composer_error accepts composer.json", packagist_composer_error("composer.json"), ""),
            expect_equal(
                "packagist_composer_error rejects other names",
                packagist_composer_error("pkg.json"),
                "packagist_deploy: invalid composer_json 'pkg.json': want a file named composer.json",
            ),
        ],
    )
