"""Unit and analysis tests for resource collections."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":resources.bzl", "resource_mapping_error", "resource_path_error")

def resources_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "resource_path_error accepts a nested logical path",
                resource_path_error("textures/logo.txt"),
                "",
            ),
            expect_equal(
                "resource_path_error accepts a top-level file",
                resource_path_error("logo.txt"),
                "",
            ),
            expect_equal(
                "resource_path_error rejects an empty path",
                resource_path_error(""),
                "must not be empty",
            ),
            expect_equal(
                "resource_path_error rejects an absolute path",
                resource_path_error("/etc/logo.txt"),
                "must be relative",
            ),
            expect_equal(
                "resource_path_error rejects a parent escape",
                resource_path_error("../evil.txt"),
                "must not contain '.' or '..'",
            ),
            expect_equal(
                "resource_path_error rejects a dotted segment",
                resource_path_error("a/./logo.txt"),
                "must not contain '.' or '..'",
            ),
            expect_equal(
                "resource_path_error rejects an empty segment",
                resource_path_error("a//logo.txt"),
                "must not contain empty segments",
            ),
            expect_equal(
                "resource_path_error rejects a trailing slash",
                resource_path_error("a/"),
                "must not contain empty segments",
            ),
            expect_equal(
                "resource_path_error rejects backslash separators",
                resource_path_error("a\\logo.txt"),
                "must use '/' separators",
            ),
            expect_equal(
                "resource_mapping_error accepts distinct paths",
                resource_mapping_error(["logo.txt", "generated/sprite.bin"]),
                "",
            ),
            expect_equal(
                "resource_mapping_error rejects a duplicate path",
                resource_mapping_error(["logo.txt", "logo.txt"]),
                "duplicate path 'logo.txt'",
            ),
            expect_equal(
                "resource_mapping_error rejects a file directory collision",
                resource_mapping_error(["data", "data/blob.bin"]),
                "path 'data' collides with 'data/blob.bin'",
            ),
            expect_equal(
                "resource_mapping_error rejects a reversed collision",
                resource_mapping_error(["data/blob.bin", "data"]),
                "path 'data' collides with 'data/blob.bin'",
            ),
            expect_equal(
                "resource_mapping_error accepts a shared prefix without boundary",
                resource_mapping_error(["data.bin", "data/blob.bin"]),
                "",
            ),
        ],
    )

EXPECTED_RESOURCES_PLAIN_OBSERVATIONS = """subject //deploy/rules:resources_demo_plain
file logo.txt
file sprite.bin
field count=2
field paths=generated/sprite.bin,logo.txt
field processor=
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:resources_demo_plain
aspect_field transitive_count=0"""

EXPECTED_RESOURCES_PROCESSED_OBSERVATIONS = """subject //deploy/rules:resources_demo_processed
file logo.txt
file sprite.bin
field count=2
field paths=generated/sprite.bin,logo.txt
field processor=//deploy/rules:resources_demo_processor
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:resources_demo_processed
aspect_field transitive_count=0"""

def resources_plain_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":resources_demo_plain"],
        expected_observations = EXPECTED_RESOURCES_PLAIN_OBSERVATIONS,
    )

def resources_processed_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":resources_demo_processed"],
        expected_observations = EXPECTED_RESOURCES_PROCESSED_OBSERVATIONS,
    )

EXPECTED_CONSUMER_PLAIN_OBSERVATIONS = """subject //deploy/tests/fixtures/consumer_resources:consumer_plain
file banner.txt
file sprite.bin
field count=2
field paths=banner.txt,generated/sprite.bin
field processor=
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/tests/fixtures/consumer_resources:consumer_plain
aspect_field transitive_count=0"""

def resources_consumer_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":consumer_plain"],
        expected_observations = EXPECTED_CONSUMER_PLAIN_OBSERVATIONS,
    )
