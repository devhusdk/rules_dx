"""Hermetic expected-failure proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _starlark_failure_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_starlark_failure_test = analysistest.make(
    _starlark_failure_impl,
    expect_failure = True,
    config_settings = {"//command_line_option:collect_code_coverage": False},
    attrs = {
        "expected_failure_substring": attr.string(
            doc = "Substring the expected analysis failure message must contain.",
            mandatory = True,
        ),
    },
)

def failure_test(name, target, expected_failure_substring, **kwargs):
    """Asserts one analysis-time fail() fires with the expected diagnostic."""
    kwargs.setdefault("size", "small")
    _starlark_failure_test(
        name = name,
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
        **kwargs
    )
