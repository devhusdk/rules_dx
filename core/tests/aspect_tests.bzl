"""Analysis tests proving aspect subjects."""

load("//core:defs.bzl", "starlark_test")

EXPECTED_ASPECT_OBSERVATIONS = """subject //core/tests:aspect_group_under_test
file aspect_group_under_test.txt
field dep_count=2
field note=group-note
aspect_field aspect_seen=True
aspect_field field_count=2
aspect_field has_subject=True
aspect_field subject_label=//core/tests:aspect_group_under_test
aspect_field transitive=//core/tests:aspect_leaf_under_test,//core/tests:subject_under_test
aspect_field transitive_count=2
subject //core/tests:aspect_leaf_under_test
file aspect_leaf_under_test.txt
field left=19
field right=23
field sum=42
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//core/tests:aspect_leaf_under_test
aspect_field transitive_count=0"""

def aspect_subject_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":aspect_leaf_under_test", ":aspect_group_under_test"],
        expected_observations = EXPECTED_ASPECT_OBSERVATIONS,
    )
