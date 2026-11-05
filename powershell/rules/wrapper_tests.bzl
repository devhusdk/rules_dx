"""Coverage wrapper tests for PowerShell."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

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

def pwsh_coverage_env_test(name, target):
    """Asserts a PowerShell test lists its coverage sources under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
