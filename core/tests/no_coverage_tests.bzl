"""Analysis tests that no_coverage_transition turns coverage off on its edge."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS")
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

_LauncherEdgeInfo = provider(doc = "Whether the native launcher attr went through a transition.", fields = ["transitioned"])

def _launcher_probe_impl(ctx):
    return [_LauncherEdgeInfo(transitioned = type(ctx.attr._exec_launcher) == "list")]

_launcher_probe = rule(
    implementation = _launcher_probe_impl,
    attrs = NATIVE_LAUNCHER_ATTRS,
)

def _launcher_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, target[_LauncherEdgeInfo].transitioned, "native launcher attr has no transition")
    return analysistest.end(env)

_launcher_test = analysistest.make(
    _launcher_impl,
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_CoveredBuildScriptsInfo = provider(doc = "cargo_build_script targets configured with coverage on.", fields = ["labels"])

def _dep_targets(value):
    if type(value) == "Target":
        return [value]
    if type(value) == "list":
        return [item for item in value if type(item) == "Target"]
    if type(value) == "dict":
        return [item for item in list(value.keys()) + list(value.values()) if type(item) == "Target"]
    return []

def _covered_build_scripts_aspect_impl(target, ctx):
    transitive = []
    for name in dir(ctx.rule.attr):
        for dep in _dep_targets(getattr(ctx.rule.attr, name)):
            if _CoveredBuildScriptsInfo in dep:
                transitive.append(dep[_CoveredBuildScriptsInfo].labels)
    direct = [str(target.label)] if ctx.rule.kind == "cargo_build_script" and ctx.configuration.coverage_enabled else []
    return [_CoveredBuildScriptsInfo(labels = depset(direct, transitive = transitive))]

_covered_build_scripts_aspect = aspect(
    implementation = _covered_build_scripts_aspect_impl,
    attr_aspects = ["*"],
)

def _build_scripts_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.equals(env, [], target[_CoveredBuildScriptsInfo].labels.to_list(), "cargo_build_script configured with coverage on")
    return analysistest.end(env)

_build_scripts_test = analysistest.make(
    _build_scripts_impl,
    config_settings = {"//command_line_option:collect_code_coverage": True},
    extra_target_under_test_aspects = [_covered_build_scripts_aspect],
)

def no_coverage_build_scripts_test(name, target_under_test, size, **kwargs):
    """Fails when target_under_test, built with coverage on, configures a cargo_build_script with coverage on."""
    _build_scripts_test(name = name, target_under_test = target_under_test, size = size, **kwargs)

def no_coverage_tests(name):
    _reader(name = name + "_reader")
    _probe(name = name + "_probe", dep = ":" + name + "_reader")
    _off_test(name = name + "_edge", size = "small", target_under_test = ":" + name + "_probe")
    _launcher_probe(name = name + "_launcher_probe")
    _launcher_test(name = name + "_launcher", size = "small", target_under_test = ":" + name + "_launcher_probe")
    native.test_suite(name = name, tests = [":" + name + "_edge", ":" + name + "_launcher"])
