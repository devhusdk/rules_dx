"""Unit and analysis tests for the Octopus Deploy publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":octopus.bzl", "OCTOPUS_DEFAULT_URL", "octopus_channel_error", "octopus_environment_error", "octopus_package_error", "octopus_project_error", "octopus_schema_error", "octopus_space_error", "octopus_url_error", "octopus_version_error")

def octopus_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "octopus_schema_error accepts the v1 launcher-safe charset",
                octopus_schema_error(),
                "",
            ),
            expect_equal(
                "octopus_project_error accepts the demo project",
                octopus_project_error("octopus_demo"),
                "",
            ),
            expect_equal(
                "octopus_project_error accepts dotted projects",
                octopus_project_error("Dx.Web"),
                "",
            ),
            expect_equal(
                "octopus_project_error rejects an empty project",
                octopus_project_error(""),
                "octopus_deploy: invalid project '': want a non-empty " +
                "project (for example 'octopus_demo')",
            ),
            expect_equal(
                "octopus_project_error rejects launcher-unsafe project characters",
                octopus_project_error("dx;curl evil"),
                "octopus_deploy: invalid project 'dx;curl evil': want " +
                "only [A-Za-z0-9._-] so the project embeds safely in the " +
                "deploy launcher",
            ),
            expect_equal(
                "octopus_channel_error accepts the default channel",
                octopus_channel_error("Default"),
                "",
            ),
            expect_equal(
                "octopus_channel_error rejects an empty channel",
                octopus_channel_error(""),
                "octopus_deploy: invalid channel '': want a non-empty " +
                "channel (for example 'Default')",
            ),
            expect_equal(
                "octopus_channel_error rejects launcher-unsafe channel characters",
                octopus_channel_error("ch;curl evil"),
                "octopus_deploy: invalid channel 'ch;curl evil': want " +
                "only [A-Za-z0-9._-] so the channel embeds safely in the " +
                "deploy launcher",
            ),
            expect_equal(
                "octopus_version_error accepts the demo version",
                octopus_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "octopus_version_error accepts prerelease versions",
                octopus_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "octopus_version_error rejects an empty version",
                octopus_version_error(""),
                "octopus_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "octopus_version_error rejects launcher-unsafe version characters",
                octopus_version_error("1.0;curl evil"),
                "octopus_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+] so the version embeds safely in " +
                "the deploy launcher",
            ),
            expect_equal(
                "octopus_environment_error accepts the demo environment",
                octopus_environment_error("Production"),
                "",
            ),
            expect_equal(
                "octopus_environment_error rejects an empty environment",
                octopus_environment_error(""),
                "octopus_deploy: invalid environment '': want a non-empty " +
                "environment (for example 'Production')",
            ),
            expect_equal(
                "octopus_environment_error rejects launcher-unsafe environment characters",
                octopus_environment_error("prod;curl evil"),
                "octopus_deploy: invalid environment 'prod;curl evil': want " +
                "only [A-Za-z0-9._-] so the environment embeds safely in " +
                "the deploy launcher",
            ),
            expect_equal(
                "octopus_space_error accepts the default empty space",
                octopus_space_error(""),
                "",
            ),
            expect_equal(
                "octopus_space_error accepts a named space",
                octopus_space_error("Default"),
                "",
            ),
            expect_equal(
                "octopus_url_error accepts the default placeholder URL",
                octopus_url_error(OCTOPUS_DEFAULT_URL),
                "",
            ),
            expect_equal(
                "octopus_url_error rejects plaintext URLs",
                octopus_url_error("http://octopus.example.invalid/"),
                "octopus_deploy: invalid octopus_url 'http://octopus.example.invalid/': " +
                "want an https URL so pushes never go over plaintext",
            ),
            expect_equal(
                "octopus_package_error accepts an archive tarball filename",
                octopus_package_error("release_demo.tar.gz"),
                "",
            ),
            expect_equal(
                "octopus_package_error rejects a non-tarball filename",
                octopus_package_error("release_demo.zip"),
                "octopus_deploy: invalid package 'release_demo.zip': want a " +
                "filename ending in .tar.gz (archive_deploy output)",
            ),
        ],
    )

EXPECTED_OCTOPUS_DEFAULT_OBSERVATIONS = """subject //deploy/rules:octopus_demo
file octopus_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:octopus_demo
aspect_field transitive_count=0"""

EXPECTED_OCTOPUS_DEBUG_OBSERVATIONS = """subject //deploy/rules:octopus_demo_debug
file octopus_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:octopus_demo_debug
aspect_field transitive_count=0"""

def octopus_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":octopus_demo"],
        expected_observations = EXPECTED_OCTOPUS_DEFAULT_OBSERVATIONS,
    )

def octopus_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":octopus_demo_debug"],
        expected_observations = EXPECTED_OCTOPUS_DEBUG_OBSERVATIONS,
    )
