"""Coverage wrapper tests for PowerShell."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:conformance.bzl", "coverage_env_tests")

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    sources = run_env.get("DX_POWERSHELL_COVERAGE_SOURCES", "")
    if ctx.attr.coverage:
        asserts.true(env, sources.endswith(".coverage_sources"), "no coverage source list: " + sources)
    else:
        asserts.equals(env, "", sources)
    return analysistest.end(env)

_COVERAGE = coverage_env_tests(_coverage_env_impl)
_coverage_env_on_test = _COVERAGE.on
_coverage_env_off_test = _COVERAGE.off

def pwsh_coverage_env_test(name, target):
    """Asserts a PowerShell test lists its coverage sources under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
