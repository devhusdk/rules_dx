"""Coverage proofs for the owned generated metadata corpus."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:real_aspects.bzl", "real_format_aspect", "real_lint_aspect")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _corpus_coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct_sources = target[QualitySourcesInfo].direct_sources
    asserts.true(
        env,
        "starlark" in direct_sources,
        "corpus target publishes no starlark sources",
    )
    covered = sorted([f.short_path for f in direct_sources["starlark"].to_list()])
    asserts.equals(
        env,
        covered,
        sorted(ctx.attr.owned_srcs),
        "quality source set must hold every owned Starlark file",
    )
    return analysistest.end(env)

_corpus_coverage_test = analysistest.make(
    _corpus_coverage_impl,
    extra_target_under_test_aspects = [
        real_format_aspect,
        real_lint_aspect,
    ],
    attrs = {
        "owned_srcs": attr.string_list(),
    },
)

def _owned_srcs():
    """Returns the workspace paths of the Starlark files this package owns."""
    prefix = native.package_name()
    return [prefix + "/BUILD.bazel"] + [
        prefix + "/" + f.lstrip(":")
        for f in native.glob(["*.bzl"])
    ]

def corpus_coverage_test(name, subject, **kwargs):
    """Instantiates the proof that the subject checks every owned metadata file."""
    kwargs.setdefault("size", "small")
    _corpus_coverage_test(
        name = name,
        owned_srcs = _owned_srcs(),
        target_under_test = subject,
        **kwargs
    )
