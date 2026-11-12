"""Hermetic expected-failure proofs."""

load("@bazel_skylib//lib:dicts.bzl", "dicts")
load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _starlark_failure_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

def failure_test_rule(config_settings = {}, extra_target_under_test_aspects = []):
    """Returns an expected-failure analysis test rule that applies `config_settings` and `extra_target_under_test_aspects`."""
    return analysistest.make(
        _starlark_failure_impl,
        expect_failure = True,
        config_settings = dicts.add({"//command_line_option:collect_code_coverage": False}, config_settings),
        extra_target_under_test_aspects = extra_target_under_test_aspects,
        attrs = {
            "expected_failure_substring": attr.string(
                doc = "Substring the expected analysis failure message must contain.",
                mandatory = True,
            ),
        },
    )

_starlark_failure_test = failure_test_rule()

def failure_test(name, target, expected_failure_substring, **kwargs):
    """Asserts one analysis-time fail() fires with the expected diagnostic."""
    kwargs.setdefault("size", "small")
    _starlark_failure_test(
        name = name,
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
        **kwargs
    )
