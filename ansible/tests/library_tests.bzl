"""ansible_library ownership proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _mixed_split_impl(ctx):
    env = analysistest.begin(ctx)
    direct = analysistest.target_under_test(env)[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["ansible", "yaml"], sorted(direct.keys()))
    asserts.equals(env, ["main.yml"], [f.basename for f in direct["ansible"].to_list()])
    asserts.equals(env, ["data.yml"], [f.basename for f in direct["yaml"].to_list()])
    return analysistest.end(env)

mixed_split_test = analysistest.make(_mixed_split_impl)

def _plain_yaml_impl(ctx):
    env = analysistest.begin(ctx)
    direct = analysistest.target_under_test(env)[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["yaml"], sorted(direct.keys()))
    return analysistest.end(env)

plain_yaml_test = analysistest.make(_plain_yaml_impl)
