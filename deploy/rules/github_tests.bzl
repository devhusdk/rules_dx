"""Unit and analysis tests for the GitHub Release publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":github.bzl", "github_asset_collision_error", "github_tag_error")

def github_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "github_tag_error accepts the dry-run placeholder tag",
                github_tag_error("v0.0.0-dryrun"),
                "",
            ),
            expect_equal(
                "github_tag_error accepts dotted release tags",
                github_tag_error("v1.2.3"),
                "",
            ),
            expect_equal(
                "github_tag_error rejects an empty tag",
                github_tag_error(""),
                "github_deploy: invalid tag '': want a non-empty tag " +
                "(for example 'v0.0.0-dryrun')",
            ),
            expect_equal(
                "github_tag_error rejects shell-unsafe tag characters",
                github_tag_error("v1.0;curl evil"),
                "github_deploy: invalid tag 'v1.0;curl evil': want only " +
                "[A-Za-z0-9._-] so the tag embeds safely in the deploy launcher",
            ),
            expect_equal(
                "github_asset_collision_error accepts unique asset names",
                github_asset_collision_error([
                    ("//linux:app", "app-linux.tar.gz"),
                    ("//mac:app", "app-mac.tar.gz"),
                ]),
                "",
            ),
            expect_equal(
                "github_asset_collision_error names both artifacts that share an asset name",
                github_asset_collision_error([
                    ("//linux:app.tar.gz", "app.tar.gz"),
                    ("//mac:app.tar.gz", "app.tar.gz"),
                ]),
                "github_deploy: artifacts //linux:app.tar.gz and " +
                "//mac:app.tar.gz share the asset name 'app.tar.gz'",
            ),
        ],
    )

EXPECTED_GITHUB_DEFAULT_OBSERVATIONS = """subject //deploy/rules:github_demo
file github_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:github_demo
aspect_field transitive_count=0"""

EXPECTED_GITHUB_DEBUG_OBSERVATIONS = """subject //deploy/rules:github_demo_debug
file github_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:github_demo_debug
aspect_field transitive_count=0"""

def github_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":github_demo"],
        expected_observations = EXPECTED_GITHUB_DEFAULT_OBSERVATIONS,
    )

def github_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":github_demo_debug"],
        expected_observations = EXPECTED_GITHUB_DEBUG_OBSERVATIONS,
    )
