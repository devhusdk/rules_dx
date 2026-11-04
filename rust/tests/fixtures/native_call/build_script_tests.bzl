"""Analysis test for the C/C++ tool identity of cargo build script actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _relative(value):
    if value.startswith("${pwd}/"):
        return value[len("${pwd}/"):]
    return value

def _tool_inputs_test_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [
        a
        for a in analysistest.target_actions(env)
        if a.mnemonic == "CargoBuildScriptRun"
    ]
    asserts.equals(env, 1, len(actions))
    action = actions[0]
    inputs = {f.path: True for f in action.inputs.to_list()}
    for name in ["CC", "CXX", "AR"]:
        value = _relative(action.env.get(name, ""))
        asserts.true(env, value != "", "{} is not set".format(name))
        asserts.false(env, value.startswith("/"), "{} is absolute: {}".format(name, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    return analysistest.end(env)

build_script_tool_inputs_test = analysistest.make(_tool_inputs_test_impl)
