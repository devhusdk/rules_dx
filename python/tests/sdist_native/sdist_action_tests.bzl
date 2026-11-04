"""Analysis tests that the selected compiler decides the toolchain env of a native sdist build."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_COMPILER = str(Label("//cc/toolchains:compiler"))
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
        if path.startswith("/usr/") or path.startswith("/lib"):
            paths.append(path)
    return paths

def _llvm_impl(ctx):
    env = analysistest.begin(ctx)
    action = _action(env)
    if action:
        asserts.true(env, "/llvm" in action.env["CC"] or "llvm+" in action.env["CC"], "CC outside the llvm repo: " + action.env["CC"])
        for name in _FLAGS:
            asserts.true(env, action.env.get(name), name + " is empty")
        asserts.false(env, "--cross" in action.argv, "native build ran in cross mode")
        for name, value in action.env.items():
            asserts.equals(env, [], _host_paths(value), "host path in " + name)
    return analysistest.end(env)

def _gcc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _action(env)
    if action:
        asserts.true(env, "dx_gcc_linux_x86_64" in action.env["CC"], "CC outside the pinned GCC: " + action.env["CC"])
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
    for compiler, rule in [("llvm", _llvm_test), ("gcc", _gcc_test)]:
        test = "%s_%s" % (name, compiler)
        rule(
            name = test,
            size = "small",
            testonly = True,
            target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests, testonly = True)
