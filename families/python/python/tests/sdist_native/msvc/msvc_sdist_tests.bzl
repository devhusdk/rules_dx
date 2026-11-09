"""Analysis tests that an MSVC sdist build runs cl.exe from the toolchain and fails on a non-Windows executor."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_py//uv/private/pep517_whl:cc_layer.bzl", "cc_layer_test_util")
load("@rules_py//uv/private/pep517_whl:pep517_native_whl.bzl", "pep517_native_whl")

_MARKER = cc_layer_test_util.execroot_marker

def _action(env):
    found = [a for a in analysistest.target_actions(env) if a.mnemonic == "PySdistNativeBuild"]
    asserts.equals(env, 1, len(found), "expected one PySdistNativeBuild action")
    return found[0] if found else None

def _entries(action, name):
    return action.env.get(name, "").split(";")

def _runs_cl_impl(ctx):
    env = analysistest.begin(ctx)
    action = _action(env)
    if action:
        asserts.true(env, "--msvc" in action.argv, "missing --msvc: %s" % action.argv)
        asserts.false(env, "--toolchain-flags" in action.argv, "GNU toolchain flags requested")
        asserts.false(env, "--cross" in action.argv, "native build ran in cross mode")
        for name in ["CC", "CXX", "AR", "LD", "STRIP", "CFLAGS", "LDSHAREDFLAGS"]:
            asserts.false(env, name in action.env, name + " is set for cl.exe")
        for name in ["PATH", "INCLUDE", "LIB"]:
            entries = _entries(action, name)
            asserts.true(env, len(entries) > 1, name + " is empty")
            for entry in entries:
                asserts.true(env, entry.startswith(_MARKER + "\\"), "%s entry without the execroot marker: %s" % (name, entry))
            asserts.true(env, "msvc_layout\\" in entries[0], name + " carries no layout directory: " + entries[0])
        asserts.true(env, _entries(action, "INCLUDE")[-1].endswith("\\include"), "INCLUDE lacks the interpreter include dir")
        asserts.true(env, _entries(action, "LIB")[-1].endswith("\\libs"), "LIB lacks the interpreter libs dir")
        inputs = [f.path for f in action.inputs.to_list()]
        asserts.true(env, any([p.endswith("libs/python312.lib") for p in inputs]), "python312.lib is not an input")
        asserts.true(env, any([p.endswith("include/pyconfig.h") for p in inputs]), "pyconfig.h is not an input")
    return analysistest.end(env)

def _linux_exec_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "MSVC sdist builds need a Windows execution platform matching the target")
    return analysistest.end(env)

_WINDOWS_X64 = [str(Label(":windows_x64"))]
_SETTINGS = {
    "//command_line_option:platforms": _WINDOWS_X64,
    "//command_line_option:extra_toolchains": [
        str(Label(":cc_toolchain")),
        str(Label(":python_toolchain")),
        str(Label(":native_build_toolchain")),
    ],
}

_runs_cl_test = analysistest.make(
    _runs_cl_impl,
    config_settings = _SETTINGS | {"//command_line_option:extra_execution_platforms": _WINDOWS_X64},
)
_linux_exec_test = analysistest.make(_linux_exec_impl, expect_failure = True, config_settings = _SETTINGS)

def msvc_sdist_tests(name, sdist, tool):
    """Declares the stub sdist build and its two analysis tests.

    Args:
        name: Name of the test suite.
        sdist: Stub sdist archive.
        tool: Stub build tool.
    """
    fixture = name + "_fixture"
    pep517_native_whl(
        name = fixture,
        src = sdist,
        tags = ["manual"],
        tool = tool,
        version = "0.0.1",
    )
    tests = [name + "_windows_exec", name + "_linux_exec"]
    _runs_cl_test(name = tests[0], size = "small", testonly = True, target_under_test = ":" + fixture)
    _linux_exec_test(name = tests[1], size = "small", testonly = True, target_under_test = ":" + fixture)
    native.test_suite(name = name, tests = tests, testonly = True)
