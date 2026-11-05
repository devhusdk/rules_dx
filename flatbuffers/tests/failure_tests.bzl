"""Analysis-failure tests for the FlatBuffers codegen rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//flatbuffers/rules:codegen.bzl", "FlatBuffersJavaStageInfo")

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

def _java_stage_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[FlatBuffersJavaStageInfo]
    roots = info.stage_roots.to_list()
    staged = info.staged.to_list()
    asserts.equals(env, 2, len(roots))
    for root in roots:
        asserts.true(env, root.endswith("_fbs_java_stage/base") or root.endswith("_fbs_java_stage/api"), "include root: " + root)
    names = sorted([f.short_path for f in staged if f.extension == "fbs"])
    asserts.true(env, len(names) >= 3, "closure not staged: " + str(names))
    for f in staged:
        asserts.true(env, "_fbs_java_stage/" in f.path, "unstaged input " + f.path)
    return analysistest.end(env)

flatbuffers_java_stage_test = analysistest.make(_java_stage_impl)
