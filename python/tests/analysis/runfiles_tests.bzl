"""Analysis tests for what a python target carries in its runfiles."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _runfiles_names(env):
    target = analysistest.target_under_test(env)
    return [f.basename for f in target[DefaultInfo].default_runfiles.files.to_list()]

def _declared_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, "dep.py" in _runfiles_names(env), "declared dependency missing from runfiles")
    return analysistest.end(env)

def _undeclared_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.false(env, "dep.py" in _runfiles_names(env), "undeclared dependency found in runfiles")
    return analysistest.end(env)

declared_dep_test = analysistest.make(_declared_impl)
undeclared_dep_test = analysistest.make(_undeclared_impl)

def _no_click_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    bad = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list() if "/click/" in f.short_path]
    asserts.equals(env, [], bad, "click found in runfiles")
    return analysistest.end(env)

no_click_runfiles_test = analysistest.make(_no_click_impl)
