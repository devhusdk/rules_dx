"""Analysis tests for FlatBuffers include staging."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//flatbuffers/private:codegen.bzl", "FlatBuffersCcStageInfo", "FlatBuffersJavaStageInfo")
load("//flatbuffers/private:defs.bzl", "FlatBuffersInfo")

def _stage_args_impl(ctx):
    env = analysistest.begin(ctx)
    action = [a for a in analysistest.target_actions(env) if a.mnemonic == "FlatcRust"][0]
    argv = action.argv
    roots = [argv[i + 1] for i in range(len(argv) - 1) if argv[i] == "-I"]
    asserts.equals(env, 2, len(roots))
    for root in roots:
        asserts.true(env, root.endswith("/_fbs_stage"), "include root: " + root)
    asserts.false(env, "." in argv, "argv has a bare '.'")
    for f in action.inputs.to_list():
        if f.extension == "fbs":
            asserts.true(env, "_fbs_stage/" in f.path, "unstaged input " + f.path)
    return analysistest.end(env)

flatbuffers_stage_args_test = analysistest.make(_stage_args_impl)

def _undeclared_rust_impl(ctx):
    env = analysistest.begin(ctx)
    action = [a for a in analysistest.target_actions(env) if a.mnemonic == "FlatcRust"][0]
    names = [f.basename for f in action.inputs.to_list()]
    asserts.true(env, "lost.fbs" in names, "declared schema missing: " + str(names))
    asserts.false(env, "hidden.fbs" in names, "undeclared include is readable: " + str(names))
    return analysistest.end(env)

flatbuffers_undeclared_rust_test = analysistest.make(_undeclared_rust_impl)

def _undeclared_java_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[FlatBuffersJavaStageInfo]
    names = [f.basename for f in info.staged.to_list()]
    asserts.true(env, "lost.fbs" in names, "declared schema missing: " + str(names))
    asserts.false(env, "hidden.fbs" in names, "undeclared include is readable: " + str(names))
    return analysistest.end(env)

flatbuffers_undeclared_java_test = analysistest.make(_undeclared_java_impl)

def _java_stage_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[FlatBuffersJavaStageInfo]
    roots = info.stage_roots.to_list()
    staged = info.staged.to_list()
    asserts.equals(env, 2, len(roots))
    for root in roots:
        asserts.true(env, root.endswith("/_fbs_stage"), "include root: " + root)
    names = sorted([f.short_path for f in staged if f.extension == "fbs"])
    asserts.equals(env, 2, len(names), "each schema staged once: " + str(names))
    for f in staged:
        asserts.true(env, "/_fbs_stage/" in f.path, "unstaged input " + f.path)
    return analysistest.end(env)

flatbuffers_java_stage_test = analysistest.make(_java_stage_impl)

def _cc_stage_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[FlatBuffersCcStageInfo]
    roots = info.stage_roots.to_list()
    staged = info.staged.to_list()
    asserts.equals(env, 2, len(roots))
    for root in roots:
        asserts.true(env, root.endswith("/_fbs_stage"), "include root: " + root)
    names = sorted([f.short_path for f in staged if f.extension == "fbs"])
    asserts.equals(env, 2, len(names), "each schema staged once: " + str(names))
    for f in staged:
        asserts.true(env, "_fbs_stage/" in f.path, "unstaged input " + f.path)
    return analysistest.end(env)

flatbuffers_cc_stage_test = analysistest.make(_cc_stage_impl)

def _chain_stage_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[FlatBuffersInfo]
    symlinks = [a for a in analysistest.target_actions(env) if a.mnemonic == "Symlink"]
    asserts.equals(env, ["c.fbs"], [f.basename for a in symlinks for f in a.outputs.to_list()])
    asserts.equals(env, ["a.fbs", "b.fbs", "c.fbs"], sorted([f.basename for f in info.staged.to_list()]))
    asserts.equals(env, 1, len(info.stage_roots.to_list()))
    return analysistest.end(env)

flatbuffers_chain_stage_test = analysistest.make(_chain_stage_impl)
