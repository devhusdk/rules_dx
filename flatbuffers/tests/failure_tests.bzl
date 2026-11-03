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

def _stage_args_impl(ctx):
    env = analysistest.begin(ctx)
    action = [a for a in analysistest.target_actions(env) if a.mnemonic == "FlatcRust"][0]
    argv = action.argv
    i = argv.index("-I")
    asserts.true(env, argv[i + 1].endswith("shapes_rs_generated_fbs_stage"), "include root: " + argv[i + 1])
    asserts.false(env, "." in argv, "argv has a bare '.'")
    for f in action.inputs.to_list():
        if f.extension == "fbs":
            asserts.true(env, "_fbs_stage/" in f.path, "unstaged input " + f.path)
    return analysistest.end(env)

flatbuffers_stage_args_test = analysistest.make(_stage_args_impl)
