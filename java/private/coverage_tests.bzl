"""Analysis tests for the JVM test coverage environment."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":coverage.bzl", "DX_JVM_TEST_RUNNER")
load("//core:conformance.bzl", "coverage_env_tests")

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    want = ctx.attr.main_class if ctx.attr.coverage else None
    asserts.equals(env, want, run_env.get("JACOCO_MAIN_CLASS"))
    if ctx.attr.coverage:
        asserts.true(env, InstrumentedFilesInfo in target, "no InstrumentedFilesInfo")
    return analysistest.end(env)

_COVERAGE = coverage_env_tests(_coverage_env_impl, attrs = {"main_class": attr.string()})
_coverage_env_on_test = _COVERAGE.on
_coverage_env_off_test = _COVERAGE.off

def jvm_coverage_env_test(name, target, main_class = DX_JVM_TEST_RUNNER):
    """Asserts a JVM test sets JACOCO_MAIN_CLASS to main_class under coverage, which the Windows launcher leaves out."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target, main_class = main_class)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target, main_class = main_class)
