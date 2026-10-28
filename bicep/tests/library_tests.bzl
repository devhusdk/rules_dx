"""bicep_module ownership and provider proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//bicep/rules:providers.bzl", "BicepInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_bicep_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["bicep"], sorted(direct.keys()))
    asserts.equals(env, ["app.bicep"], [f.basename for f in direct["bicep"].to_list()])
    return analysistest.end(env)

owns_bicep_class_test = analysistest.make(_owns_bicep_class_impl)

def _transitive_context_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[BicepInfo]
    asserts.equals(env, ["app.bicep", "lib.bicep"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, ["data.json"], [f.basename for f in info.transitive_data.to_list()])
    return analysistest.end(env)

transitive_context_test = analysistest.make(_transitive_context_impl)
