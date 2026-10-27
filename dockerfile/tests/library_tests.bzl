"""dockerfile_library ownership proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_dockerfile_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["dockerfile"], sorted(direct.keys()))
    asserts.equals(
        env,
        ["app.Dockerfile"],
        [f.basename for f in direct["dockerfile"].to_list()],
    )
    return analysistest.end(env)

owns_dockerfile_class_test = analysistest.make(_owns_dockerfile_class_impl)
