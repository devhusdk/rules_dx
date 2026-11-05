"""Analysis test that a foreign cmake target rejects an unknown generator."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _unknown_generator_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "`Bogus` is not a known generator")
    return analysistest.end(env)

unknown_generator_test = analysistest.make(
    _unknown_generator_impl,
    expect_failure = True,
)
