"""Wrapper conformance tests for Perl."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:conformance.bzl", "coverage_env_tests")

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    collected = [p for p in runfiles if p.endswith("perl/private/coverage/DxLcov.pm")]
    if ctx.attr.coverage:
        asserts.equals(env, "-MDxLcov", run_env.get("PERL5OPT"))
        asserts.true(env, run_env.get("PERL5LIB", "").endswith("perl/private/coverage"), "PERL5LIB lacks the collector: " + run_env.get("PERL5LIB", ""))
        asserts.true(env, collected != [], "runfiles lack DxLcov.pm")
        asserts.true(env, InstrumentedFilesInfo in target, "no InstrumentedFilesInfo")
    else:
        asserts.equals(env, None, run_env.get("PERL5OPT"))
        asserts.equals(env, None, run_env.get("PERL5LIB"))
        asserts.equals(env, [], collected)
    return analysistest.end(env)

_COVERAGE = coverage_env_tests(_coverage_env_impl)
_coverage_env_on_test = _COVERAGE.on
_coverage_env_off_test = _COVERAGE.off

def perl_coverage_env_test(name, target):
    """Asserts a Perl test loads the DxLcov collector under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
