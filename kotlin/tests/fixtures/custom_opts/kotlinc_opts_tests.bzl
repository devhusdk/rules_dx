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
