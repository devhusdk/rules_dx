"""ansible_library ownership proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:owns_class_test.bzl", "owns_class_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo")

mixed_split_test = owns_class_test("ansible", ["main.yml"], others = {"yaml": ["data.yml"]})

def _plain_yaml_impl(ctx):
    env = analysistest.begin(ctx)
    direct = analysistest.target_under_test(env)[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["yaml"], sorted(direct.keys()))
    return analysistest.end(env)

plain_yaml_test = analysistest.make(_plain_yaml_impl)
