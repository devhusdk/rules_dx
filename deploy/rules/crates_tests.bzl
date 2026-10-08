"""Unit and analysis tests for the crates.io publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":crates.bzl", "crates_allow_dirty_error", "crates_file_error", "crates_name_error", "crates_schema_error", "crates_version_error")

def crates_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "crates_schema_error accepts the v1 launcher-safe charset",
                crates_schema_error(),
                "",
            ),
            expect_equal(
                "crates_name_error accepts the demo crate name",
                crates_name_error("crates_demo"),
                "",
            ),
            expect_equal(
                "crates_name_error accepts dashed names",
                crates_name_error("dx-plugin"),
                "",
            ),
            expect_equal(
                "crates_name_error rejects an empty name",
                crates_name_error(""),
                "crates_deploy: invalid crate name '': want a non-empty " +
                "name (for example 'crates_demo')",
            ),
            expect_equal(
                "crates_name_error rejects dots in crate names",
                crates_name_error("dx.plugin"),
                "crates_deploy: invalid crate name 'dx.plugin': want " +
                "only [A-Za-z0-9_-] so the name embeds safely in the deploy launcher",
            ),
            expect_equal(
                "crates_name_error rejects launcher-unsafe name characters",
                crates_name_error("dx;curl evil"),
                "crates_deploy: invalid crate name 'dx;curl evil': want " +
                "only [A-Za-z0-9_-] so the name embeds safely in the deploy launcher",
            ),
            expect_equal(
                "crates_version_error accepts the demo version",
                crates_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "crates_version_error accepts prerelease versions",
                crates_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "crates_version_error rejects an empty version",
                crates_version_error(""),
                "crates_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "crates_version_error rejects launcher-unsafe version characters",
                crates_version_error("1.0;curl evil"),
                "crates_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+] so the version embeds safely in the deploy launcher",
            ),
            expect_equal(
                "crates_file_error accepts a manifest filename",
                crates_file_error("Cargo.toml"),
                "",
            ),
            expect_equal(
                "crates_file_error rejects launcher-unsafe filenames",
                crates_file_error("Cargo evil.toml"),
                "crates_deploy: invalid crate file 'Cargo evil.toml': want " +
                "no quotes, backslashes, spaces, or newlines so the filename " +
                "embeds safely in the deploy launcher",
            ),
            expect_equal(
                "crates_allow_dirty_error accepts a clean tree",
                crates_allow_dirty_error(False),
                "",
            ),
            expect_equal(
                "crates_allow_dirty_error rejects dirty trees without approval",
                crates_allow_dirty_error(True),
                "crates_deploy: allow_dirty=True requires explicit owner " +
                "approval; keep a clean tree and publish from committed " +
                "sources instead",
            ),
        ],
    )

EXPECTED_CRATES_DEFAULT_OBSERVATIONS = """subject //deploy/rules:crates_demo
file crates_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:crates_demo
aspect_field transitive_count=0"""

EXPECTED_CRATES_DEBUG_OBSERVATIONS = """subject //deploy/rules:crates_demo_debug
file crates_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:crates_demo_debug
aspect_field transitive_count=0"""

def crates_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":crates_demo"],
        expected_observations = EXPECTED_CRATES_DEFAULT_OBSERVATIONS,
    )

def crates_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":crates_demo_debug"],
        expected_observations = EXPECTED_CRATES_DEBUG_OBSERVATIONS,
    )
