"""Analysis tests that no_coverage_transition turns coverage off on its edge."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:no_coverage.bzl", "no_coverage_transition")

_CoverageSeenInfo = provider(doc = "Coverage state a target was built with.", fields = ["enabled"])

def _reader_impl(ctx):
    return [_CoverageSeenInfo(enabled = ctx.configuration.coverage_enabled)]

_reader = rule(implementation = _reader_impl)

def _probe_impl(ctx):
    return [
        _CoverageSeenInfo(enabled = ctx.attr.dep[0][_CoverageSeenInfo].enabled),
        DefaultInfo(),
    ]

_probe = rule(
    implementation = _probe_impl,
    attrs = {"dep": attr.label(cfg = no_coverage_transition)},
)

def _off_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.false(env, target[_CoverageSeenInfo].enabled, "dependency built with coverage on")
    return analysistest.end(env)

_off_test = analysistest.make(
    _off_impl,
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

def no_coverage_tests(name):
    _reader(name = name + "_reader")
    _probe(name = name + "_probe", dep = ":" + name + "_reader")
    _off_test(name = name, size = "small", target_under_test = ":" + name + "_probe")
