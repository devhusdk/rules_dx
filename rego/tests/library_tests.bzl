"""rego_library ownership and provider proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//rego/rules:providers.bzl", "RegoInfo")

def _owns_rego_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["rego"], sorted(direct.keys()))
    asserts.equals(env, ["app.rego"], [f.basename for f in direct["rego"].to_list()])
    return analysistest.end(env)

owns_rego_class_test = analysistest.make(_owns_rego_class_impl)

def _transitive_context_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[RegoInfo]
    asserts.equals(env, ["app.rego", "lib.rego"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, ["data.json"], [f.basename for f in info.transitive_data.to_list()])
    return analysistest.end(env)

transitive_context_test = analysistest.make(_transitive_context_impl)
