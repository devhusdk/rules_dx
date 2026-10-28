"""cloudformation_template proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_cloudformation_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["cloudformation"], sorted(direct.keys()))
    asserts.equals(
        env,
        ["template.yaml"],
        [f.basename for f in direct["cloudformation"].to_list()],
    )
    return analysistest.end(env)

owns_cloudformation_class_test = analysistest.make(_owns_cloudformation_class_impl)
