"""Analysis tests for the Kotlin test coverage environment."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    want = "com.google.testing.junit.runner.BazelTestRunner" if ctx.attr.coverage else None
    asserts.equals(env, want, run_env.get("JACOCO_MAIN_CLASS"))
    if ctx.attr.coverage:
        asserts.true(env, InstrumentedFilesInfo in target, "no InstrumentedFilesInfo")
    return analysistest.end(env)

_coverage_env_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"coverage": attr.bool(default = True)},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_env_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"coverage": attr.bool(default = False)},
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def kotlin_coverage_env_test(name, target):
    """Asserts a Kotlin test names JACOCO_MAIN_CLASS under coverage, which the Windows launcher leaves out."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
