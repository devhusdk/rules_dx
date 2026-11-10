"""Analysis test factory for name-matched library rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":sources.bzl", "QualitySourcesInfo")

def owns_class_test(class_id, basenames):
    """Returns an analysis test proving a library owns exactly `basenames` in `class_id`."""

    def _impl(ctx):
        env = analysistest.begin(ctx)
        target = analysistest.target_under_test(env)
        direct = target[QualitySourcesInfo].direct_sources
        asserts.equals(env, [class_id], sorted(direct.keys()))
        asserts.equals(env, basenames, [f.basename for f in direct[class_id].to_list()])
        return analysistest.end(env)

    return analysistest.make(_impl)
