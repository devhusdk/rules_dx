"""Failure proofs for consumer-selected C/C++ lint policy."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:real_aspects.bzl", "real_cc_lint_aspect")

_CANONICAL = "@@"  # buildifier: disable=canonical-repository

def _cc_subject_impl(ctx):
    """Exposes one C/C++ lint subject under the transitioned policy."""
    return [DefaultInfo(files = depset([]))]

_cc_subject = rule(
    implementation = _cc_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [real_cc_lint_aspect],
            mandatory = True,
        ),
    },
)

def _rejected_impl(ctx):
    """Asserts the expected declared-policy rejection diagnostic."""
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_cc_policy_rejects_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_CANONICAL + "//config:workspace": _CANONICAL + "//quality/tests/fixtures/consumer_policy:cc_declared_policy"},
)

def cc_policy_failure_tests(name, cases):
    """Instantiates one rejection proof per unhinted C/C++ subject."""
    for case in cases:
        _cc_subject(
            name = name + "_" + case["case"] + "_subject",
            target = case["subject"],
            tags = ["manual"],
        )
        _cc_policy_rejects_test(
            name = name + "_" + case["case"] + "_test",
            size = "small",
            target_under_test = name + "_" + case["case"] + "_subject",
            expected_failure_substring = case["expected_failure"],
        )
