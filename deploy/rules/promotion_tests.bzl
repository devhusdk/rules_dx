"""Unit and analysis tests for the promotion publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":promotion.bzl", "promotion_artifact_error", "promotion_edge_error", "promotion_environment_error", "promotion_schema_error", "promotion_version_error")

def promotion_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "promotion_schema_error accepts the v1 launcher-safe charset",
                promotion_schema_error(),
                "",
            ),
            expect_equal(
                "promotion_environment_error accepts the staging source",
                promotion_environment_error("staging"),
                "",
            ),
            expect_equal(
                "promotion_environment_error accepts the production target",
                promotion_environment_error("production"),
                "",
            ),
            expect_equal(
                "promotion_environment_error rejects an empty environment",
                promotion_environment_error(""),
                "promotion_deploy: invalid environment '': want a non-empty " +
                "environment (for example 'staging')",
            ),
            expect_equal(
                "promotion_environment_error rejects launcher-unsafe environment characters",
                promotion_environment_error("prod;curl evil"),
                "promotion_deploy: invalid environment 'prod;curl evil': want " +
                "only [A-Za-z0-9._-] so the environment embeds safely in the " +
                "deploy launcher",
            ),
            expect_equal(
                "promotion_version_error accepts the demo version",
                promotion_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "promotion_version_error accepts prerelease versions",
                promotion_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "promotion_version_error rejects an empty version",
                promotion_version_error(""),
                "promotion_deploy: invalid version '': want a non-empty " +
                "version (for example '1.2.3')",
            ),
            expect_equal(
                "promotion_version_error rejects launcher-unsafe version characters",
                promotion_version_error("1.0;curl evil"),
                "promotion_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+] so the version embeds safely in the " +
                "deploy launcher",
            ),
            expect_equal(
                "promotion_edge_error accepts the staging to production edge",
                promotion_edge_error("staging", "production"),
                "",
            ),
            expect_equal(
                "promotion_edge_error rejects a self edge",
                promotion_edge_error("production", "production"),
                "promotion_deploy: invalid edge 'production -> production': " +
                "source and target environments must differ",
            ),
            expect_equal(
                "promotion_artifact_error accepts an archive tarball filename",
                promotion_artifact_error("release_demo.tar.gz"),
                "",
            ),
            expect_equal(
                "promotion_artifact_error accepts an image tar filename",
                promotion_artifact_error("oci_demo.tar"),
                "",
            ),
            expect_equal(
                "promotion_artifact_error rejects an unknown suffix",
                promotion_artifact_error("release_demo.exe"),
                "promotion_deploy: invalid artifact 'release_demo.exe': want " +
                "one of .tar.gz, .tar, .tgz, .whl, .jar, .nupkg, .zip",
            ),
            expect_equal(
                "promotion_artifact_error rejects launcher-unsafe filenames",
                promotion_artifact_error("release demo.tar.gz"),
                "promotion_deploy: invalid artifact 'release demo.tar.gz': want " +
                "no quotes, backslashes, spaces, or newlines so the filename " +
                "embeds safely in the deploy launcher",
            ),
        ],
    )

EXPECTED_PROMOTION_DEFAULT_OBSERVATIONS = """subject //deploy/rules:promotion_demo
file promotion_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:promotion_demo
aspect_field transitive_count=0"""

EXPECTED_PROMOTION_DEBUG_OBSERVATIONS = """subject //deploy/rules:promotion_demo_debug
file promotion_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:promotion_demo_debug
aspect_field transitive_count=0"""

def promotion_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":promotion_demo"],
        expected_observations = EXPECTED_PROMOTION_DEFAULT_OBSERVATIONS,
    )

def promotion_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":promotion_demo_debug"],
        expected_observations = EXPECTED_PROMOTION_DEBUG_OBSERVATIONS,
    )
