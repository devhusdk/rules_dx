"""Analysis tests that the selected compiler decides the toolchain env of a native sdist build."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_COMPILER = str(Label("@rules_dx//cc/toolchains:compiler"))
_DEP_GROUP = str(Label("@rules_py//uv/private/constraints/dep_group:dep_group"))
_FLAGS = ["CFLAGS", "LDSHAREDFLAGS"]

def _action(env):
    found = [a for a in analysistest.target_actions(env) if a.mnemonic == "PySdistNativeBuild"]
    asserts.equals(env, 1, len(found), "expected one PySdistNativeBuild action")
    return found[0] if found else None

def _host_paths(value):
    paths = []
    for token in value.split(" "):
        path = token.split("=")[-1]
        for prefix in ("-I", "-L", "-isystem"):
            path = path.removeprefix(prefix)
        if path.startswith("/usr/") or path.startswith("/lib") or path.startswith("/Library/Developer"):
            paths.append(path)
    return paths

def _common(env, action):
    for name in _FLAGS:
        asserts.true(env, action.env.get(name), name + " is empty")
    asserts.false(env, "--cross" in action.argv, "native build ran in cross mode")
    for name, value in action.env.items():
        asserts.equals(env, [], _host_paths(value), "host path in " + name)

def _llvm_impl(ctx):
    env = analysistest.begin(ctx)
    action = _action(env)
    if action:
        cc = action.env.get("CC", "")
        asserts.true(env, "/llvm" in cc or "llvm+" in cc, "CC outside the llvm repo: " + cc)
        for name, value in action.env.items():
            asserts.false(env, "dx_gcc" in value, "GCC path in " + name)
        _common(env, action)
    return analysistest.end(env)

def _gcc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _action(env)
    if action:
        for name in ["CC", "CXX", "AR"]:
            value = action.env.get(name, "")
            asserts.true(env, "dx_gcc_linux_x86_64" in value, name + " outside the pinned GCC: " + value)
        cc = action.env.get("CC", "").removeprefix("$(EXECROOT)/")
        inputs = [f.path for f in action.inputs.to_list()]
        asserts.true(env, cc in inputs, "CC is not an action input: " + cc)
        _common(env, action)
    return analysistest.end(env)

_llvm_test = analysistest.make(_llvm_impl, config_settings = {_COMPILER: "llvm", _DEP_GROUP: "sdist-native"})
_gcc_test = analysistest.make(_gcc_impl, config_settings = {_COMPILER: "gcc", _DEP_GROUP: "sdist-native"})

def sdist_action_tests(name, target):
    """Asserts the toolchain env of the native sdist build under each compiler.

    Args:
        name: Name of the test suite.
        target: The pep517_native_whl target of an sdist-only package.
    """
    tests = []
    linux = ["@platforms//os:linux", "@platforms//cpu:x86_64"]
    macos = ["@platforms//os:macos", "@platforms//cpu:aarch64"]
    for compiler, rule, constraints in [("llvm", _llvm_test, linux), ("gcc", _gcc_test, linux), ("llvm_macos", _llvm_test, macos)]:
        test = "%s_%s" % (name, compiler)
        rule(
            name = test,
            size = "small",
            testonly = True,
            target_compatible_with = constraints,
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests, testonly = True)
