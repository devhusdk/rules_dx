"""Analysis test that the default ghcopts follow //config:config_profile."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":defs.bzl", "DEFAULT_GHCOPTS", "STANDARD_GHCOPTS", "STRICT_GHCOPTS")

def _opts_impl(ctx):
    return [DefaultInfo(files = depset(), runfiles = ctx.runfiles()), _OptsInfo(opts = ctx.attr.opts)]

_OptsInfo = provider(fields = ["opts"])

_opts = rule(
    implementation = _opts_impl,
    attrs = {"opts": attr.string_list()},
)

def _strict_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, STRICT_GHCOPTS, analysistest.target_under_test(env)[_OptsInfo].opts)
    return analysistest.end(env)

def _standard_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, STANDARD_GHCOPTS, analysistest.target_under_test(env)[_OptsInfo].opts)
    return analysistest.end(env)

_strict_test = analysistest.make(
    _strict_impl,
    config_settings = {str(Label("//config:config_profile")): "strict"},
)

_standard_test = analysistest.make(
    _standard_impl,
    config_settings = {str(Label("//config:config_profile")): "standard"},
)

def profile_ghcopts_tests(name):
    """Declares tests that the default ghcopts select on the profile."""
    _opts(name = name + "_opts", opts = DEFAULT_GHCOPTS, tags = ["manual"])
    _strict_test(name = name + "_strict", target_under_test = ":" + name + "_opts")
    _standard_test(name = name + "_standard", target_under_test = ":" + name + "_opts")
