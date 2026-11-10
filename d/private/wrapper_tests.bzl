"""Coverage analysis tests for D test wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    if ctx.attr.coverage:
        upstream = run_env.get("DX_D_COVERAGE_UPSTREAM", "")
        asserts.true(env, upstream.endswith("/" + ctx.attr.upstream), "upstream is " + upstream)
        asserts.true(env, InstrumentedFilesInfo in target, "no InstrumentedFilesInfo")
    else:
        asserts.false(env, "DX_D_COVERAGE_UPSTREAM" in run_env, "launcher env set without coverage")
    return analysistest.end(env)

_ATTRS = {
    "coverage": attr.bool(),
    "upstream": attr.string(),
}

_cov_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = _ATTRS,
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_cov_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = _ATTRS,
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def d_coverage_env_test(name, target, upstream, **kwargs):
    """Asserts a D test runs under the LCOV launcher with coverage on, and only then."""
    _cov_on_test(name = name, size = "small", target_under_test = target, coverage = True, upstream = upstream, **kwargs)
    _cov_off_test(name = name + "_off", size = "small", target_under_test = target, coverage = False, upstream = upstream, **kwargs)
