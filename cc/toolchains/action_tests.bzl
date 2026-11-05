"""Analysis tests that the selected compiler decides the tools of compile and link actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_COMPILER = str(Label("//cc/toolchains:compiler"))

def _actions(env, mnemonics):
    found = [a for a in analysistest.target_actions(env) if a.mnemonic in mnemonics]
    asserts.true(env, len(found) > 0, "no action with mnemonic in %s: %s" % (mnemonics, [a.mnemonic for a in analysistest.target_actions(env)]))
    return found

def _gcc_impl(ctx):
    env = analysistest.begin(ctx)
    for action in _actions(env, ctx.attr.mnemonics):
        asserts.true(env, "dx_gcc_linux_x86_64" in action.argv[0], "tool outside the pinned GCC: " + action.argv[0])
        for arg in action.argv:
            asserts.false(env, arg.startswith("/usr/") or arg.startswith("/lib"), "host path in %s: %s" % (action.mnemonic, arg))
        if action.mnemonic == "CppLink":
            linkers = [f for f in action.inputs.to_list() if "dx_gcc_linux_x86_64" in f.path and f.path.endswith("/xbin/ld")]
            asserts.true(env, len(linkers) > 0, "link action does not use the pinned GCC linker")
    return analysistest.end(env)

def _gcc_arm64_impl(ctx):
    env = analysistest.begin(ctx)
    for action in _actions(env, ctx.attr.mnemonics):
        asserts.true(env, "dx_gcc_linux_aarch64_cross" in action.argv[0], "tool outside the pinned cross GCC: " + action.argv[0])
        asserts.false(env, "dx_gcc_linux_x86_64" in action.argv[0], "x86_64 GCC used for arm64: " + action.argv[0])
        for arg in action.argv:
            asserts.false(env, arg.startswith("/usr/") or arg.startswith("/lib"), "host path in %s: %s" % (action.mnemonic, arg))
    return analysistest.end(env)

def _llvm_impl(ctx):
    env = analysistest.begin(ctx)
    for action in _actions(env, ctx.attr.mnemonics):
        asserts.false(env, "dx_gcc" in action.argv[0], "GCC tool under llvm: " + action.argv[0])
    return analysistest.end(env)

_ATTRS = {"mnemonics": attr.string_list()}

_gcc_test = analysistest.make(_gcc_impl, attrs = _ATTRS, config_settings = {_COMPILER: "gcc"})
_gcc_arm64_test = analysistest.make(
    _gcc_arm64_impl,
    attrs = _ATTRS,
    config_settings = {_COMPILER: "gcc", "//command_line_option:platforms": [str(Label("//cc/tests/gcc_cross:linux_arm64"))]},
)
_llvm_test = analysistest.make(_llvm_impl, attrs = _ATTRS, config_settings = {_COMPILER: "llvm"})

def cc_action_tests(name, compile_target, link_target):
    """Asserts compile and link actions use the pinned GCC under gcc and not under llvm.

    Args:
        name: Name of the test suite.
        compile_target: A cc_library with C sources.
        link_target: A cc_binary.
    """
    tests = []
    for kind, target, mnemonics in [("compile", compile_target, ["CppCompile"]), ("link", link_target, ["CppLink"])]:
        for compiler, rule in [("gcc", _gcc_test), ("gcc_arm64", _gcc_arm64_test), ("llvm", _llvm_test)]:
            test = "%s_%s_%s" % (name, kind, compiler)
            rule(
                name = test,
                size = "small",
                mnemonics = mnemonics,
                testonly = True,
                target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
                target_under_test = target,
            )
            tests.append(test)
    native.test_suite(name = name, tests = tests, testonly = True)
