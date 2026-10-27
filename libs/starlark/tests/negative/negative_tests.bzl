"""Negative demonstrations as green hermetic proofs."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:failure_test.bzl", "failure_test")
load("//rust/rules:defs.bzl", "rust_test")

def _red_manifest(name):
    """Exposes one red runner's harness manifest so a demo can stage it."""
    native.filegroup(
        name = name + "_manifest",
        srcs = [":" + name],
        output_group = "starlark_manifest",
        testonly = True,
        tags = ["manual"],
    )

def red_failing_checks(name):
    """Red unit runner with two wrong expects and one passing control."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("deliberately wrong sum", 2, 3),
            expect_equal("deliberately wrong product", 4, 5),
            expect_equal("control that still passes", 2, 2),
        ],
        tags = ["manual"],
    )
    _red_manifest(name)

_RED_OBSERVATIONS = """subject //libs/starlark/tests/negative:negative_subject
file negative_subject.txt
field left=0
field right=0
field sum=43
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//libs/starlark/tests/negative:negative_subject
aspect_field transitive_count=0"""

def red_missing_observation(name):
    """Red analysis runner with one wrong observation field."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":negative_subject"],
        expected_observations = _RED_OBSERVATIONS,
        tags = ["manual"],
    )
    _red_manifest(name)

def red_missing_fragment(name):
    """Red execution runner wanting one absent substring."""
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            ":present_fixture.txt": "this substring is absent",
        },
        tags = ["manual"],
    )
    _red_manifest(name)

def failing_check_demo(name):
    rust_test(
        name = name,
        srcs = ["negative_proof_test.rs"],
        data = [
            ":red_failing_checks",
            ":red_failing_checks_manifest",
        ],
        env = {
            "DX_NEGATIVE_CASE": "failing_check",
            "DX_RUNNER_BIN": "$(rootpath :red_failing_checks)",
            "STARLARK_HARNESS_MANIFEST": "$(rlocationpath :red_failing_checks_manifest)",
        },
        tags = ["no-coverage"],
        target_compatible_with = ["@platforms//os:linux"],
        deps = ["//tools/testing:dx_testing"],
    )

def missing_observation_demo(name):
    rust_test(
        name = name,
        srcs = ["negative_proof_test.rs"],
        data = [
            ":red_missing_observation",
            ":red_missing_observation_manifest",
        ],
        env = {
            "DX_NEGATIVE_CASE": "missing_observation",
            "DX_RUNNER_BIN": "$(rootpath :red_missing_observation)",
            "STARLARK_HARNESS_MANIFEST": "$(rlocationpath :red_missing_observation_manifest)",
        },
        tags = ["no-coverage"],
        target_compatible_with = ["@platforms//os:linux"],
        deps = ["//tools/testing:dx_testing"],
    )

def missing_fragment_demo(name):
    rust_test(
        name = name,
        srcs = ["negative_proof_test.rs"],
        data = [
            ":red_missing_fragment",
            ":red_missing_fragment_manifest",
        ],
        env = {
            "DX_NEGATIVE_CASE": "missing_fragment",
            "DX_RUNNER_BIN": "$(rootpath :red_missing_fragment)",
            "STARLARK_HARNESS_MANIFEST": "$(rlocationpath :red_missing_fragment_manifest)",
        },
        tags = ["no-coverage"],
        target_compatible_with = ["@platforms//os:linux"],
        deps = ["//tools/testing:dx_testing"],
    )

def _wrong_phase_subject_impl(ctx):
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (load mode): subjects must be empty: " +
             "load tests observe loading, not analysis")
    return [DefaultInfo(files = depset([]))]

_wrong_phase_subject = rule(
    implementation = _wrong_phase_subject_impl,
    attrs = {
        "subjects": attr.label_list(),
    },
)

def wrong_phase_demo(name):
    _wrong_phase_subject(
        name = name + "_subject",
        subjects = [":negative_subject"],
        tags = ["manual"],
    )
    failure_test(
        name = name,
        target = ":" + name + "_subject",
        expected_failure_substring = "starlark_test (load mode): subjects must be empty",
    )
