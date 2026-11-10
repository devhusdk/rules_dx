"""Analysis tests proving configuration subjects."""

load("//core:defs.bzl", "starlark_test")

EXPECTED_CONFIG_OBSERVATIONS = """subject //core/tests:config_group_under_test
file config_group_under_test.txt
field config_value=plain
field dep_config=flipped
field dep_note=flipped-note
field note=plain-note
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//core/tests:config_group_under_test
aspect_field transitive=//core/tests:config_leaf_under_test
aspect_field transitive_count=1
config_field config_value=plain
config_field dep_config=flipped
config_field dep_note=flipped-note
config_field fragment_platform=present
config_field note=plain-note
config_field transition=group
subject //core/tests:config_leaf_under_test
file config_leaf_under_test.txt
field config_value=plain
field note=plain-note
aspect_field aspect_seen=True
aspect_field field_count=2
aspect_field has_subject=True
aspect_field subject_label=//core/tests:config_leaf_under_test
aspect_field transitive_count=0
config_field config_value=plain
config_field fragment_platform=present
config_field note=plain-note
config_field transition=leaf"""

def config_subject_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":config_leaf_under_test", ":config_group_under_test"],
        expected_observations = EXPECTED_CONFIG_OBSERVATIONS,
    )
