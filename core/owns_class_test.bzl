"""Analysis test factory for name-matched library rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core/quality:sources.bzl", "QualitySourcesInfo")

def owns_class_test(class_id, basenames, others = {}, sort = False):
    """Returns an analysis test proving a library owns exactly `basenames` in `class_id`.

    Args:
      class_id: quality class the library must own.
      basenames: expected basenames in `class_id`.
      others: further owned classes, mapped to their expected basenames.
      sort: compare sorted basenames instead of depset order.
    """
    expected = dict(others)
    expected[class_id] = basenames

    def _impl(ctx):
        env = analysistest.begin(ctx)
        target = analysistest.target_under_test(env)
        direct = target[QualitySourcesInfo].direct_sources
        asserts.equals(env, sorted(expected.keys()), sorted(direct.keys()))
        for cls, names in expected.items():
            actual = [f.basename for f in direct[cls].to_list()] if cls in direct else []
            asserts.equals(env, names, sorted(actual) if sort else actual)
        return analysistest.end(env)

    return analysistest.make(_impl)
