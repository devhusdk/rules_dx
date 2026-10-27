"""Analysis test that pins a wrapper's direct quality sources."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _direct_sources_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    sources = target[QualitySourcesInfo].direct_sources
    owned = {key: sorted([f.basename for f in sources[key].to_list()]) for key in sources if sources[key].to_list()}
    expected = {key: sorted(value) for key, value in ctx.attr.expected.items()}
    asserts.equals(env, expected, owned)
    return analysistest.end(env)

_direct_sources_test = analysistest.make(
    _direct_sources_test_impl,
    attrs = {
        "expected": attr.string_list_dict(doc = "File class to the basenames the wrapper lints directly."),
    },
)

def dx_direct_sources_test(name, target_under_test, expected, **kwargs):
    """Asserts a wrapper's direct quality sources by file class."""
    _direct_sources_test(
        name = name,
        target_under_test = target_under_test,
        expected = expected,
        **kwargs
    )
