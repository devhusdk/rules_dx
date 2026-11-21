"""Analysis tests for cargo_build_script targets: coverage and the cc flags they get."""

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

def _raw_impl(ctx):
    env = analysistest.begin(ctx)
    runs = [a for a in analysistest.target_actions(env) if a.mnemonic == "CargoBuildScriptRun"]
    asserts.equals(env, 1, len(runs), "CargoBuildScriptRun actions")
    for action in runs:
        asserts.false(env, "GCOV_GCNO_FILE" in action.env, "CargoBuildScriptRun env sets GCOV_GCNO_FILE")
    return analysistest.end(env)

_raw_build_script_coverage_env_test = analysistest.make(
    _raw_impl,
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

def raw_build_script_coverage_env_test(name, target_under_test, size, **kwargs):
    """Fails when the CargoBuildScriptRun action of target_under_test sets GCOV_GCNO_FILE under coverage."""
    _raw_build_script_coverage_env_test(name = name, target_under_test = target_under_test, size = size, **kwargs)

_PREFIX_MAP = "-ffile-prefix-map="

_CxxFlagsInfo = provider(doc = "crate_universe build script runs and those that remap the execroot for C but not C++.", fields = ["runs", "unmapped"])

def _cxxflags_aspect_impl(target, ctx):
    runs = []
    unmapped = []
    for name in dir(ctx.rule.attr):
        for dep in _dep_targets(getattr(ctx.rule.attr, name)):
            if _CxxFlagsInfo in dep:
                runs.append(dep[_CxxFlagsInfo].runs)
                unmapped.append(dep[_CxxFlagsInfo].unmapped)
    direct_runs = []
    direct_unmapped = []
    if ctx.rule.kind == "cargo_build_script":
        for action in target.actions:
            if action.mnemonic != "CargoBuildScriptRun":
                continue
            direct_runs.append(str(target.label))

            # cc-rs reads CXXFLAGS, not CFLAGS, for C++ sources, so both must remap the execroot.
            if (_PREFIX_MAP in action.env.get("CFLAGS", "")) != (_PREFIX_MAP in action.env.get("CXXFLAGS", "")):
                direct_unmapped.append(str(target.label))
    return [_CxxFlagsInfo(
        runs = depset(direct_runs, transitive = runs),
        unmapped = depset(direct_unmapped, transitive = unmapped),
    )]

_cxxflags_aspect = aspect(
    implementation = _cxxflags_aspect_impl,
    attr_aspects = ["*"],
)

def _cxxflags_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[_CxxFlagsInfo]
    asserts.true(env, len(info.runs.to_list()) > 0, "no CargoBuildScriptRun reached")
    asserts.equals(env, [], info.unmapped.to_list(), "build scripts whose CFLAGS and CXXFLAGS disagree on " + _PREFIX_MAP)
    return analysistest.end(env)

_build_script_cxxflags_test = analysistest.make(
    _cxxflags_impl,
    extra_target_under_test_aspects = [_cxxflags_aspect],
)

def build_script_cxxflags_test(name, target_under_test, size, **kwargs):
    """Fails when a build script under target_under_test remaps the execroot for C but not C++."""
    _build_script_cxxflags_test(name = name, target_under_test = target_under_test, size = size, **kwargs)
