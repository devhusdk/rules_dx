"""Analysis tests for forced warnings-as-errors on custom kotlinc options."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions")

def _werror_test_impl(ctx):
    env = analysistest.begin(ctx)
    opts = analysistest.target_under_test(env)[KotlincOptions]
    asserts.equals(env, "error", opts.warn)
    asserts.true(env, opts.x_no_param_assertions)
    return analysistest.end(env)

werror_test = analysistest.make(_werror_test_impl)

def _flag_off_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, "report", analysistest.target_under_test(env)[KotlincOptions].warn)
    return analysistest.end(env)

flag_off_test = analysistest.make(
    _flag_off_test_impl,
    config_settings = {str(Label("@rules_dx//config:warnings_as_errors")): False},
)

def _default_werror_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, "error", analysistest.target_under_test(env)[KotlincOptions].warn)
    return analysistest.end(env)

default_werror_test = analysistest.make(
    _default_werror_test_impl,
    config_settings = {str(Label("@rules_dx//config:warnings_as_errors")): True},
)
