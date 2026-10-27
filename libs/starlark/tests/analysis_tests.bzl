"""Analysis tests for the example subject rule."""

load("//libs/starlark:defs.bzl", "starlark_test")

EXPECTED_OBSERVATIONS = """subject //libs/starlark/tests:subject_under_test
file subject_under_test.txt
field left=40
field right=2
field sum=42
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//libs/starlark/tests:subject_under_test
aspect_field transitive_count=0"""

def subject_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":subject_under_test"],
        expected_observations = EXPECTED_OBSERVATIONS,
    )

def _runner_block(name):
    """Renders the observation block of one starlark_test target in this package."""
    label = "//libs/starlark/tests:" + name
    return "\n".join([
        "subject " + label,
        "file " + name,
        "aspect_field aspect_seen=True",
        "aspect_field field_count=0",
        "aspect_field has_subject=False",
        "aspect_field subject_label=" + label,
        "aspect_field transitive_count=0",
    ])

RUNNER_SHAPE_OBSERVATIONS = "\n".join([
    _runner_block("arithmetic_load"),
    _runner_block("arithmetic_unit"),
    _runner_block("fixture_execution"),
    _runner_block("subject_analysis"),
])

def runner_shape_tests(name):
    """Pins that every mode's executable is the native harness, not a script."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":arithmetic_load",
            ":arithmetic_unit",
            ":fixture_execution",
            ":subject_analysis",
        ],
        expected_observations = RUNNER_SHAPE_OBSERVATIONS,
    )

def observations_only_tests(name):
    """Runs an analysis test whose only evidence is the observations dump."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":subject_under_test"],
    )
