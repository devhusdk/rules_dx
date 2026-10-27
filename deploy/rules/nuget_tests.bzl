"""Unit and analysis tests for the NuGet publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":nuget.bzl", "NUGET_DEFAULT_SOURCE", "nuget_id_error", "nuget_nupkg_error", "nuget_source_error", "nuget_version_error")

def nuget_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "nuget_id_error accepts the demo package id",
                nuget_id_error("nuget_demo"),
                "",
            ),
            expect_equal(
                "nuget_id_error accepts dotted ids",
                nuget_id_error("Dx.Plugin"),
                "",
            ),
            expect_equal(
                "nuget_id_error rejects an empty id",
                nuget_id_error(""),
                "nuget_deploy: invalid package id '': want a non-empty " +
                "id (for example 'nuget_demo')",
            ),
            expect_equal(
                "nuget_id_error rejects launcher-unsafe id characters",
                nuget_id_error("dx;curl evil"),
                "nuget_deploy: invalid package id 'dx;curl evil': want " +
                "only [A-Za-z0-9._-] so the id embeds safely in the deploy launcher",
            ),
            expect_equal(
                "nuget_version_error accepts the demo version",
                nuget_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "nuget_version_error accepts prerelease versions",
                nuget_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "nuget_version_error rejects an empty version",
                nuget_version_error(""),
                "nuget_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "nuget_version_error rejects launcher-unsafe version characters",
                nuget_version_error("1.0;curl evil"),
                "nuget_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+] so the version embeds safely in the deploy launcher",
            ),
            expect_equal(
                "nuget_nupkg_error accepts a pinned nupkg filename",
                nuget_nupkg_error("nuget_demo.0.0.0.nupkg"),
                "",
            ),
            expect_equal(
                "nuget_nupkg_error rejects a non-nupkg filename",
                nuget_nupkg_error("nuget_demo.0.0.0.zip"),
                "nuget_deploy: invalid nupkg 'nuget_demo.0.0.0.zip': want a " +
                "filename ending in .nupkg",
            ),
            expect_equal(
                "nuget_source_error accepts the default source",
                nuget_source_error(NUGET_DEFAULT_SOURCE),
                "",
            ),
            expect_equal(
                "nuget_source_error rejects plaintext URLs",
                nuget_source_error("http://example.com/v3/index.json"),
                "nuget_deploy: invalid source 'http://example.com/v3/index.json': " +
                "want an https URL so pushes never go over plaintext",
            ),
            expect_equal(
                "nuget_source_error rejects an empty source",
                nuget_source_error(""),
                "nuget_deploy: invalid source '': want a non-empty https " +
                "URL (for example '" + NUGET_DEFAULT_SOURCE + "')",
            ),
        ],
    )
