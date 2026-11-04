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
    for name in ["CC", "CXX", "AR", "HOST_CC", "HOST_CXX", "HOST_AR", "TARGET_CC", "TARGET_CXX", "TARGET_AR"]:
        value = _relative(action.env.get(name, ""))
        asserts.true(env, value != "", "{} is not set".format(name))
        asserts.false(env, value.startswith("/"), "{} is absolute: {}".format(name, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    return analysistest.end(env)

build_script_tool_inputs_test = analysistest.make(_tool_inputs_test_impl)

_COMPILER = str(Label("//cc/toolchains:compiler"))

def _run_action(env):
    actions = [
        a
        for a in analysistest.target_actions(env)
        if a.mnemonic == "CargoBuildScriptRun"
    ]
    asserts.equals(env, 1, len(actions))
    return actions[0]

def _gcc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    inputs = {f.path: True for f in action.inputs.to_list()}
    for name in ["CC", "CXX", "AR"]:
        value = _relative(action.env.get(name, ""))
        asserts.true(env, "dx_gcc_linux_x86_64" in value, "{} is not the pinned GCC: {}".format(name, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    for name, value in action.env.items():
        if name in ["CC", "CXX", "AR"]:
            continue
        for part in value.split(" "):
            asserts.false(env, part.startswith("/usr") or part.startswith("-L/usr") or part.startswith("-L/lib"), "host path in {}: {}".format(name, value))
    return analysistest.end(env)

def _llvm_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    for name in ["CC", "CXX", "AR"]:
        value = action.env.get(name, "")
        asserts.false(env, "dx_gcc" in value, "{} is GCC under llvm: {}".format(name, value))
    return analysistest.end(env)

_gcc_test = analysistest.make(_gcc_impl, config_settings = {_COMPILER: "gcc"})
_llvm_test = analysistest.make(_llvm_impl, config_settings = {_COMPILER: "llvm"})

def build_script_compiler_tests(name, target):
    """Asserts the build script runs with the pinned GCC under gcc and with other tools under llvm.

    Args:
        name: Name of the test suite.
        target: A cargo_build_script that uses the cc toolchain.
    """
    tests = []
    for compiler, rule in [("gcc", _gcc_test), ("llvm", _llvm_test)]:
        test = "{}_{}".format(name, compiler)
        rule(
            name = test,
            size = "small",
            target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests)

cross_build_script_tool_inputs_test = analysistest.make(
    _tool_inputs_test_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_arm64"))},
)
