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

def _msvc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    inputs = {f.path: True for f in action.inputs.to_list()}
    for name, tool in [("CC", "cl.exe"), ("CXX", "cl.exe"), ("AR", "lib.exe")]:
        value = _relative(action.env.get(name, ""))
        asserts.true(env, value.lower().endswith(tool), "{} is not {}: {}".format(name, tool, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    include = action.env.get("INCLUDE", "")
    asserts.true(env, include != "", "INCLUDE is not set")
    for part in include.split(";"):
        asserts.true(env, part.startswith("external/") or part.startswith("${pwd}/external/") or "dx_msvc" in part, "INCLUDE entry outside the provisioned SDK: {}".format(part))
    return analysistest.end(env)

_gcc_test = analysistest.make(_gcc_impl, config_settings = {_COMPILER: "gcc"})
_llvm_test = analysistest.make(_llvm_impl, config_settings = {_COMPILER: "llvm"})
_msvc_test = analysistest.make(_msvc_impl, config_settings = {_COMPILER: "msvc"})

def _libstdcxx_impl(ctx):
    env = analysistest.begin(ctx)
    links = [
        a
        for a in analysistest.target_actions(env)
        if a.mnemonic == "Rustc"
    ]
    asserts.true(env, len(links) > 0, "no rustc action")
    for action in links:
        archives = [f.path for f in action.inputs.to_list() if f.path.endswith("/lib64/libstdc++.a")]
        asserts.true(env, len(archives) > 0, "libstdc++.a is not an input of {}".format(action.outputs.to_list()))
        for path in archives:
            asserts.true(env, "dx_gcc_linux_x86_64" in path, "libstdc++.a is not from the pinned GCC: {}".format(path))
    return analysistest.end(env)

_libstdcxx_test = analysistest.make(_libstdcxx_impl, config_settings = {_COMPILER: "gcc"})

def build_script_libstdcxx_test(name, target):
    """Asserts a binary using the C++ build script links libstdc++ from the pinned GCC.

    Args:
        name: Name of the test.
        target: A rust_test or rust_binary that depends on the build script.
    """
    _libstdcxx_test(
        name = name,
        size = "small",
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        target_under_test = target,
    )

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

def build_script_msvc_test(name, target):
    """Asserts the build script runs with the provisioned cl.exe and lib.exe on Windows x64.

    Args:
        name: Name of the test.
        target: A cargo_build_script that uses the cc toolchain.
    """
    _msvc_test(
        name = name,
        size = "small",
        target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        target_under_test = target,
    )
