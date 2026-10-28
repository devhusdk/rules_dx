"""Analysis-failure tests for the FlatBuffers codegen rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _failure_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.message)
    return analysistest.end(env)

flatbuffers_codegen_failure_test = analysistest.make(
    _failure_impl,
    expect_failure = True,
    attrs = {"message": attr.string(mandatory = True)},
)
