"""Analysis test that flutter_web_app owns its sources for quality."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_dx//core/quality:sources.bzl", "QualitySourcesInfo")

def _owns_main_impl(ctx):
    env = analysistest.begin(ctx)
    direct = analysistest.target_under_test(env)[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["dart"], sorted(direct.keys()))
    asserts.equals(env, ["main.dart"], [f.basename for f in direct["dart"].to_list()])
    return analysistest.end(env)

web_owns_main_test = analysistest.make(_owns_main_impl)
