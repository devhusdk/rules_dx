"""Wrapper conformance tests for Perl."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

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

def perl_coverage_env_test(name, target):
    """Asserts a Perl test loads the DxLcov collector under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
