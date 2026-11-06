"""Expected-failure proofs that run under the default workspace policy."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_test = analysistest.make(
    _impl,
    expect_failure = True,
    config_settings = {
        "//command_line_option:collect_code_coverage": False,
        str(Label("//config:workspace")): str(Label("//quality:default_workspace_policy")),
    },
    attrs = {
        "expected_failure_substring": attr.string(mandatory = True),
    },
)

def no_policy_failure_test(name, target, expected_failure_substring, **kwargs):
    """Asserts an analysis failure with the root workspace policy replaced by the default."""
    kwargs.setdefault("size", "small")
    _test(
        name = name,
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
        **kwargs
    )
