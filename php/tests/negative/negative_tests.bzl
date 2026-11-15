"""Analysis tests for the sources a PHP autoloader is built from."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//tools/testing:action_input.bzl", "action_input_test")

def php_autoload_input_test(name, target, file, present):
    """Checks whether the autoload action of target reads file."""
    action_input_test(name, target, "PhpAutoload", file, present)

def _failure_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_failure_test = analysistest.make(
    _failure_impl,
    expect_failure = True,
    config_settings = {"//command_line_option:collect_code_coverage": False},
    attrs = {
        "expected_failure_substring": attr.string(mandatory = True),
    },
)

def php_failure_test(name, target, expected_failure_substring):
    """Asserts that analysis of target fails with a message containing expected_failure_substring."""
    _failure_test(
        name = name,
        size = "small",
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
    )
