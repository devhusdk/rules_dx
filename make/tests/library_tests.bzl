"""make_library ownership proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_make_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["make"], sorted(direct.keys()))
    asserts.equals(
        env,
        ["Makefile", "rules.mk"],
        [f.basename for f in direct["make"].to_list()],
    )
    return analysistest.end(env)

owns_make_class_test = analysistest.make(_owns_make_class_impl)
