"""Unit and analysis tests for the GitHub Release publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":github.bzl", "github_draft_error", "github_tag_error", "tag_schema_error")

def github_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "tag_schema_error accepts the v1 launcher-safe charset",
                tag_schema_error(),
                "",
            ),
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
                "github_draft_error accepts the draft gate",
                github_draft_error(True),
                "",
            ),
            expect_equal(
                "github_draft_error rejects publishing without approval",
                github_draft_error(False),
                "github_deploy: draft=False requires explicit owner " +
                "approval; keep the draft gate and publish " +
                "the release on GitHub after approval",
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
