"""Analysis tests that the selected compiler decides the tools of compile and link actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_COMPILER = str(Label("//cc:compiler"))

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

def _gcc_arm64_native_impl(ctx):
    env = analysistest.begin(ctx)
    for action in _actions(env, ctx.attr.mnemonics):
        tool = action.argv[0]
        asserts.true(env, "dx_gcc_linux_aarch64/" in tool or "dx_gcc_linux_aarch64_16" in tool, "tool outside the pinned native GCC: " + tool)
        asserts.false(env, "_cross" in tool or "x86_64" in tool, "cross or x86_64 GCC used natively: " + tool)
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
_gcc_arm64_native_test = analysistest.make(_gcc_arm64_native_impl, attrs = _ATTRS, config_settings = {_COMPILER: "gcc"})
_llvm_test = analysistest.make(_llvm_impl, attrs = _ATTRS, config_settings = {_COMPILER: "llvm"})

_LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]
_LINUX_ARM64 = ["@platforms//os:linux", "@platforms//cpu:arm64"]

def cc_action_tests(name, compile_target, link_target):
    """Asserts compile and link actions use the pinned GCC under gcc and not under llvm.

    Args:
        name: Name of the test suite.
        compile_target: A cc_library with C sources.
        link_target: A cc_binary.
    """
    tests = []
    for kind, target, mnemonics in [("compile", compile_target, ["CppCompile"]), ("link", link_target, ["CppLink"])]:
        for compiler, rule, compatible in [
            ("gcc", _gcc_test, _LINUX_X86_64),
            ("gcc_arm64", _gcc_arm64_test, _LINUX_X86_64),
            ("gcc_arm64_native", _gcc_arm64_native_test, _LINUX_ARM64),
            ("llvm", _llvm_test, _LINUX_X86_64),
        ]:
            test = "%s_%s_%s" % (name, kind, compiler)
            rule(
                name = test,
                size = "small",
                mnemonics = mnemonics,
                testonly = True,
                target_compatible_with = compatible,
                target_under_test = target,
            )
            tests.append(test)
    native.test_suite(name = name, tests = tests, testonly = True)

_MSVC_TOOLS = {"CppCompile": "cl.exe", "CppLink": "link.exe", "CppArchive": "lib.exe"}
_MSVC_REPO = Label("@msvc_runtime//:BUILD.bazel").repo_name
_SDK_REPO = Label("@windows_sdk//:BUILD.bazel").repo_name

def _pinned(path):
    return "external/{}/".format(_MSVC_REPO) in path or "external/{}/".format(_SDK_REPO) in path

def _check_msvc(env, mnemonics, bin_dir, lib_suffix):
    for action in _actions(env, mnemonics):
        tool = action.argv[0].replace("\\", "/")
        asserts.true(env, "external/{}/".format(_MSVC_REPO) in tool, "tool outside the pinned MSVC: " + tool)
        asserts.true(env, bin_dir in tool, "tool not under %s: %s" % (bin_dir, tool))
        asserts.true(env, tool.endswith("/" + _MSVC_TOOLS[action.mnemonic]), "unexpected tool for %s: %s" % (action.mnemonic, tool))
        for key in ["INCLUDE", "LIB"]:
            for entry in (action.env or {}).get(key, "").split(";"):
                if entry:
                    entry = entry.replace("\\", "/")
                    asserts.true(env, _pinned(entry), "%s entry outside the pinned MSVC: %s" % (key, entry))
                    if key == "LIB":
                        asserts.true(env, entry.endswith(lib_suffix), "LIB entry not for %s: %s" % (lib_suffix, entry))
        for entry in (action.env or {}).get("PATH", "").split(";"):
            entry = entry.replace("\\", "/")
            asserts.false(env, not _pinned(entry) and ("Visual Studio" in entry or "Windows Kits" in entry), "PATH entry from an installed Visual Studio: " + entry)

def _msvc_impl(ctx):
    env = analysistest.begin(ctx)
    _check_msvc(env, _MSVC_TOOLS.keys(), "/bin/Hostx64/x64/", "/x64")
    return analysistest.end(env)

def _msvc_arm64_impl(ctx):
    env = analysistest.begin(ctx)
    _check_msvc(env, _MSVC_TOOLS.keys(), "/bin/Hostx64/arm64/", "/arm64")
    return analysistest.end(env)

_msvc_test = analysistest.make(_msvc_impl, config_settings = {_COMPILER: "msvc"})
_msvc_arm64_test = analysistest.make(
    _msvc_arm64_impl,
    config_settings = {_COMPILER: "msvc", "//command_line_option:platforms": [str(Label("//cc/tests/msvc_cross:windows_arm64"))]},
)

def cc_msvc_action_tests(name, compile_target, link_target):
    """Asserts compile, archive and link actions use the pinned MSVC tools under msvc.

    Args:
        name: Name of the test suite.
        compile_target: A cc_library with C sources.
        link_target: A cc_binary or cc_test.
    """
    tests = []
    for kind, target in [("compile", compile_target), ("link", link_target)]:
        test = "%s_%s_msvc" % (name, kind)
        _msvc_test(
            name = test,
            size = "small",
            testonly = True,
            target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests, testonly = True)

def cc_msvc_arm64_action_tests(name, compile_target, link_target):
    """Asserts compile, archive and link actions cross to arm64 with the pinned MSVC tools.

    Args:
        name: Name of the test suite.
        compile_target: A cc_library with C sources.
        link_target: A cc_binary or cc_test.
    """
    tests = []
    for kind, target in [("compile", compile_target), ("link", link_target)]:
        test = "%s_%s_msvc_arm64" % (name, kind)
        _msvc_arm64_test(
            name = test,
            size = "small",
            tags = ["manual"],
            testonly = True,
            exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
            target_compatible_with = ["@platforms//os:windows"],
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests, testonly = True)
