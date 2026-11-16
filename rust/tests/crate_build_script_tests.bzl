"""Analysis tests that crate_universe build scripts keep coverage features off."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//third_party/rules_rust/crate_universe:build_script_features.bzl", "NO_COVERAGE_FEATURES")

_BuildScriptsInfo = provider(doc = "crate_universe cargo_build_script targets and whether they disable coverage features.", fields = ["seen", "covered"])

def _dep_targets(value):
    if type(value) == "Target":
        return [value]
    if type(value) == "list":
        return [item for item in value if type(item) == "Target"]
    return []

def _aspect_impl(target, ctx):
    seen = []
    covered = []
    for name in dir(ctx.rule.attr):
        for dep in _dep_targets(getattr(ctx.rule.attr, name)):
            if _BuildScriptsInfo in dep:
                seen.append(dep[_BuildScriptsInfo].seen)
                covered.append(dep[_BuildScriptsInfo].covered)
    direct_seen = []
    direct_covered = []
    if ctx.rule.kind == "cargo_build_script" and "cargo-bazel" in ctx.rule.attr.tags:
        direct_seen.append(str(target.label))
        if [f for f in NO_COVERAGE_FEATURES if f not in ctx.rule.attr.features]:
            direct_covered.append(str(target.label))
    return [_BuildScriptsInfo(
        seen = depset(direct_seen, transitive = seen),
        covered = depset(direct_covered, transitive = covered),
    )]

_build_scripts_aspect = aspect(
    implementation = _aspect_impl,
    attr_aspects = ["*"],
)

def _impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[_BuildScriptsInfo]
    asserts.true(env, len(info.seen.to_list()) > 0, "no crate_universe cargo_build_script reached")
    asserts.equals(env, [], info.covered.to_list(), "crate_universe cargo_build_script without NO_COVERAGE_FEATURES")
    return analysistest.end(env)

_crate_build_scripts_test = analysistest.make(
    _impl,
    config_settings = {"//command_line_option:collect_code_coverage": True},
    extra_target_under_test_aspects = [_build_scripts_aspect],
)

def crate_build_scripts_no_coverage_test(name, target_under_test, size, **kwargs):
    """Fails when a crate_universe cargo_build_script under target_under_test lacks NO_COVERAGE_FEATURES."""
    _crate_build_scripts_test(name = name, target_under_test = target_under_test, size = size, **kwargs)
