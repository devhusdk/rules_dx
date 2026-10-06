"""Unit and analysis tests for workspace policy and applicability."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":applicability.bzl", "capability_selection", "effective_classes", "selected_adapters")

_FIXTURE_ADAPTERS = {
    "fmt-a": ["rust"],
    "lint-a": ["rust", "python"],
    "lint-b": ["rust"],
}

_FIXTURE_SECTION = {
    "format": ["fmt-a"],
    "lint": ["lint-a", "lint-b"],
    "typecheck": [],
}

def policy_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "capability_selection returns the lint selection",
                capability_selection(_FIXTURE_SECTION, "lint"),
                ["lint-a", "lint-b"],
            ),
            expect_equal(
                "capability_selection returns [] for an unselected capability",
                capability_selection(_FIXTURE_SECTION, "audit"),
                [],
            ),
            expect_equal(
                "selected_adapters preserves policy order",
                selected_adapters(["lint-b", "lint-a"], _FIXTURE_ADAPTERS),
                ["lint-b", "lint-a"],
            ),
            expect_equal(
                "selected_adapters deduplicates repeated IDs",
                selected_adapters(["lint-a", "lint-a"], _FIXTURE_ADAPTERS),
                ["lint-a"],
            ),
            expect_equal(
                "effective_classes intersects three sets sorted",
                effective_classes(
                    ["rust", "python", "markdown"],
                    ["rust", "python"],
                    ["python", "markdown"],
                ),
                ["python"],
            ),
            expect_equal(
                "effective_classes is empty without adapter support",
                effective_classes(["rust"], ["python"], ["rust", "python"]),
                [],
            ),
            expect_equal(
                "effective_classes is empty without policy authorization",
                effective_classes(["rust"], ["rust"], ["python"]),
                [],
            ),
        ],
    )

EXPECTED_POLICY_OBSERVATIONS = """subject //quality:policy_under_test
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field disabled.rust.audit=False
field disabled.rust.format=False
field disabled.rust.lint=False
field disabled.rust.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
field family.rust.audit=
field family.rust.format=fmt-a
field family.rust.lint=lint-a,lint-b
field family.rust.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=16
aspect_field has_subject=True
aspect_field subject_label=//quality:policy_under_test
aspect_field transitive_count=0"""

def policy_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":policy_under_test"],
        expected_observations = EXPECTED_POLICY_OBSERVATIONS,
    )
