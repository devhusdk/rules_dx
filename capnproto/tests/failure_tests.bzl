"""Analysis tests for capnp import staging."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _undeclared_rust_impl(ctx):
    env = analysistest.begin(ctx)
    action = [a for a in analysistest.target_actions(env) if a.mnemonic == "CapnpRust"][0]
    asserts.true(env, action.argv[0].removesuffix(".exe").endswith("/capnpc_rust_dx_subcommand"), "action runs " + action.argv[0])
    asserts.equals(env, "capnpc_rust_dx", action.argv[1])
    names = [f.basename for f in action.inputs.to_list()]
    asserts.true(env, "lost.capnp" in names, "declared schema missing: " + str(names))
    asserts.false(env, "hidden.capnp" in names, "undeclared import is readable: " + str(names))
    return analysistest.end(env)

capnp_undeclared_rust_test = analysistest.make(_undeclared_rust_impl)
