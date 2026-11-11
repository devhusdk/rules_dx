"""Analysis tests for thrift include staging."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _undeclared_rust_impl(ctx):
    env = analysistest.begin(ctx)
    action = [a for a in analysistest.target_actions(env) if a.mnemonic == "ThriftRust"][0]
    names = [f.basename for f in action.inputs.to_list()]
    asserts.true(env, "lost.thrift" in names, "declared schema missing: " + str(names))
    asserts.false(env, "hidden.thrift" in names, "undeclared include is readable: " + str(names))
    return analysistest.end(env)

thrift_undeclared_rust_test = analysistest.make(_undeclared_rust_impl)
