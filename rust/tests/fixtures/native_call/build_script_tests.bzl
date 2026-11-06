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
_LINKER_PREFERENCE = str(Label("@rules_rust//rust/settings:toolchain_linker_preference"))

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
        asserts.true(env, ctx.attr.gcc in value, "{} is not the pinned GCC: {}".format(name, value))
        asserts.false(env, "_cross" in value, "{} is a cross GCC: {}".format(name, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    for name in ["HOST_CFLAGS", "HOST_CXXFLAGS", "TARGET_CFLAGS", "TARGET_CXXFLAGS"]:
        asserts.true(env, name in action.env, "{} is not set".format(name))
    for name, value in action.env.items():
        if name in ["CC", "CXX", "AR"]:
            continue
        for part in value.split(" "):
            asserts.false(env, part.startswith("/usr") or part.startswith("-L/usr") or part.startswith("-L/lib"), "host path in {}: {}".format(name, value))
    return analysistest.end(env)

def _llvm_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    inputs = {f.path: True for f in action.inputs.to_list()}
    for name in ["CC", "CXX", "AR"]:
        value = _relative(action.env.get(name, ""))
        asserts.false(env, "dx_gcc" in value, "{} is GCC under llvm: {}".format(name, value))
        asserts.true(env, "llvm_toolchain" in value, "{} is not the LLVM toolchain: {}".format(name, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    return analysistest.end(env)

def _msvc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    inputs = {f.path: True for f in action.inputs.to_list()}
    for name, tool in [("CC", "cl.exe"), ("CXX", "cl.exe"), ("AR", "lib.exe"), ("TARGET_CC", "cl.exe"), ("TARGET_CXX", "cl.exe"), ("TARGET_AR", "lib.exe")]:
        value = _relative(action.env.get(name, ""))
        asserts.true(env, value.lower().endswith(tool), "{} is not {}: {}".format(name, tool, value))
        asserts.true(env, value in inputs, "{} is not a declared input: {}".format(name, value))
    include = action.env.get("INCLUDE", "")
    asserts.true(env, include != "", "INCLUDE is not set")
    for part in include.split(";"):
        asserts.true(env, part.startswith("external/") or part.startswith("${pwd}/external/") or "dx_msvc" in part, "INCLUDE entry outside the provisioned SDK: {}".format(part))
    return analysistest.end(env)

_GCC_ATTRS = {"gcc": attr.string(default = "dx_gcc_linux_x86_64")}

_gcc_test = analysistest.make(_gcc_impl, config_settings = {_COMPILER: "gcc"}, attrs = _GCC_ATTRS)
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
            asserts.true(env, ctx.attr.gcc in path, "libstdc++.a is not from the pinned GCC: {}".format(path))
            asserts.false(env, "_cross" in path, "libstdc++.a is from a cross GCC: {}".format(path))
    return analysistest.end(env)

_libstdcxx_test = analysistest.make(_libstdcxx_impl, config_settings = {_COMPILER: "gcc"}, attrs = _GCC_ATTRS)

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
    _libstdcxx_test(
        name = name + "_arm64",
        size = "small",
        gcc = "dx_gcc_linux_aarch64",
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:arm64"],
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
        if compiler == "gcc":
            arm64 = test + "_arm64"
            rule(
                name = arm64,
                size = "small",
                gcc = "dx_gcc_linux_aarch64",
                target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:arm64"],
                target_under_test = target,
            )
            tests.append(arm64)
    native.test_suite(name = name, tests = tests)

cross_build_script_tool_inputs_test = analysistest.make(
    _tool_inputs_test_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_arm64"))},
)

def _cross_gcc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    for name in ["CC", "CXX", "AR", "TARGET_CC", "TARGET_CXX", "TARGET_AR"]:
        value = action.env.get(name, "")
        asserts.true(env, "dx_gcc_linux_aarch64_cross" in value, "{} is not the aarch64 cross GCC: {}".format(name, value))
    for name in ["HOST_CC", "HOST_CXX", "HOST_AR"]:
        value = action.env.get(name, "")
        asserts.true(env, "dx_gcc_linux_x86_64" in value, "{} is not the exec GCC: {}".format(name, value))
        asserts.false(env, "aarch64" in value, "{} is the target toolchain: {}".format(name, value))
    cflags = action.env.get("CFLAGS", "")
    asserts.true(env, "aarch64-linux" in cflags, "CFLAGS does not target aarch64: {}".format(cflags))
    asserts.false(env, "dx_gcc_linux_x86_64" in cflags, "CFLAGS uses the exec GCC: {}".format(cflags))
    for name in ["HOST_CFLAGS", "HOST_CXXFLAGS"]:
        value = action.env.get(name, "")
        asserts.false(env, "aarch64_cross" in value, "{} has target GCC paths: {}".format(name, value))
    for name in ["TARGET_CFLAGS", "TARGET_CXXFLAGS"]:
        value = action.env.get(name, "")
        asserts.false(env, "dx_gcc_linux_x86_64" in value, "{} has exec GCC paths: {}".format(name, value))
    for name, value in action.env.items():
        for part in value.split(" "):
            asserts.false(env, part.startswith("/usr") or part.startswith("-L/usr"), "host path in {}: {}".format(name, value))
    return analysistest.end(env)

def _cross_llvm_impl(ctx):
    env = analysistest.begin(ctx)
    action = _run_action(env)
    for name in ["CFLAGS", "CXXFLAGS"]:
        parts = action.env.get(name, "").split(" ")
        asserts.true(env, "aarch64-linux-gnu" in parts, "{} does not target aarch64: {}".format(name, parts))
        asserts.false(env, "x86_64-linux-gnu" in parts, "{} targets x86_64: {}".format(name, parts))
    for name in ["HOST_CC", "HOST_CXX", "HOST_AR", "TARGET_CC", "TARGET_CXX", "TARGET_AR"]:
        value = action.env.get(name, "")
        asserts.false(env, "dx_gcc" in value, "{} is GCC under llvm: {}".format(name, value))
    return analysistest.end(env)

_CROSS = {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_arm64"))}
_cross_gcc_test = analysistest.make(_cross_gcc_impl, config_settings = dict(_CROSS, **{_COMPILER: "gcc"}))
_cross_llvm_test = analysistest.make(_cross_llvm_impl, config_settings = dict(_CROSS, **{_COMPILER: "llvm"}))

def cross_build_script_identity_tests(name, target):
    """Asserts a cross build script gets the target toolchain and HOST_* the exec toolchain.

    Args:
        name: Name of the test suite.
        target: A cargo_build_script that uses the cc toolchain.
    """
    tests = []
    for compiler, rule in [("gcc", _cross_gcc_test), ("llvm", _cross_llvm_test)]:
        test = "{}_{}".format(name, compiler)
        rule(
            name = test,
            size = "small",
            target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests)

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

def _rustc_action(env):
    actions = [
        a
        for a in analysistest.target_actions(env)
        if a.mnemonic == "Rustc"
    ]
    asserts.equals(env, 1, len(actions))
    return actions[0]

def _link_args(action):
    linker = ""
    link_args = []
    for part in action.argv:
        if part.startswith("--codegen=linker="):
            linker = part[len("--codegen=linker="):]
        elif part.startswith("-Clink-arg="):
            link_args.append(part[len("-Clink-arg="):])
        elif part.startswith("--codegen=link-arg="):
            link_args.append(part[len("--codegen=link-arg="):])
    return linker, link_args

def _link_gcc_impl(ctx):
    env = analysistest.begin(ctx)
    action = _rustc_action(env)
    linker, link_args = _link_args(action)
    asserts.true(env, ctx.attr.gcc in linker, "linker is not the pinned GCC: {}".format(linker))
    asserts.false(env, "_cross" in linker, "linker is a cross GCC: {}".format(linker))
    asserts.true(env, "-l:libstdc++.a" in link_args, "libstdc++.a is not linked: {}".format(link_args))
    asserts.true(env, "-l:libgcc.a" in link_args, "libgcc.a is not linked: {}".format(link_args))
    if "-l:libstdc++.a" in link_args and "-l:libgcc.a" in link_args:
        asserts.true(env, link_args.index("-l:libgcc.a") > link_args.index("-l:libstdc++.a"), "libgcc.a must follow libstdc++.a: {}".format(link_args))
    for arg in link_args:
        if arg.startswith("-L") or arg.startswith("-B"):
            asserts.true(env, arg[2:].startswith("external/+gcc+" + ctx.attr.gcc), "search path outside the pinned GCC: {}".format(arg))
    for part in action.argv:
        for prefix in ["/usr", "/lib", "-L/usr", "-L/lib"]:
            asserts.false(env, part.startswith(prefix), "host path in rustc argv: {}".format(part))
    return analysistest.end(env)

def _link_llvm_impl(ctx):
    env = analysistest.begin(ctx)
    linker, _ = _link_args(_rustc_action(env))
    asserts.false(env, "dx_gcc" in linker, "linker is GCC under llvm: {}".format(linker))
    return analysistest.end(env)

def _link_windows_impl(ctx):
    env = analysistest.begin(ctx)
    linker, _ = _link_args(_rustc_action(env))
    name = linker.replace("\\", "/").split("/")[-1]
    asserts.true(env, name.startswith("rust-lld"), "linker is not rust-lld: {}".format(linker))
    asserts.false(env, "clang-cl" in linker, "linker is clang-cl: {}".format(linker))
    return analysistest.end(env)

_link_gcc_test = analysistest.make(_link_gcc_impl, config_settings = {_COMPILER: "gcc"}, attrs = _GCC_ATTRS)
_link_llvm_test = analysistest.make(_link_llvm_impl, config_settings = {_COMPILER: "llvm"})

_link_windows_test = analysistest.make(_link_windows_impl, config_settings = {_COMPILER: "llvm", _LINKER_PREFERENCE: "rust"})

def rust_link_windows_test(name, target):
    """Asserts the Rust link step uses rust-lld and not clang-cl on Windows x64.

    Args:
        name: Name of the test.
        target: A rust_test or rust_binary that links native code.
    """
    _link_windows_test(
        name = name,
        size = "small",
        target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        target_under_test = target,
    )

def rust_link_compiler_tests(name, target):
    """Asserts the Rust link step uses the pinned GCC under gcc and not under llvm.

    Args:
        name: Name of the test suite.
        target: A rust_test or rust_binary that links native code.
    """
    tests = []
    for compiler, rule in [("gcc", _link_gcc_test), ("llvm", _link_llvm_test)]:
        test = "{}_{}".format(name, compiler)
        rule(
            name = test,
            size = "small",
            target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
        tests.append(test)
        if compiler == "gcc":
            arm64 = test + "_arm64"
            rule(
                name = arm64,
                size = "small",
                gcc = "dx_gcc_linux_aarch64",
                target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:arm64"],
                target_under_test = target,
            )
            tests.append(arm64)
    native.test_suite(name = name, tests = tests)

_EXTRA_RUSTC_FLAG = str(Label("@rules_rust//rust/settings:extra_rustc_flag"))
_CRT_CONFIG = {
    _COMPILER: "msvc",
    _EXTRA_RUSTC_FLAG: ["-Ctarget-feature=+crt-static"],
}

def _crt_ok_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, analysistest.target_under_test(env) != None)
    return analysistest.end(env)

def _crt_bad_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "MSVC CRT mismatch")
    return analysistest.end(env)

_crt_ok_test = analysistest.make(_crt_ok_impl, config_settings = _CRT_CONFIG)
_crt_target_flag_test = analysistest.make(_crt_ok_impl, config_settings = {_COMPILER: "msvc"})
_crt_bad_test = analysistest.make(_crt_bad_impl, config_settings = _CRT_CONFIG, expect_failure = True)

def rust_msvc_crt_tests(name, ok, bad, target_flag):
    """Asserts +crt-static Rust accepts /MT C deps and rejects /MD ones on MSVC.

    Args:
        name: Name of the test suite.
        ok: A rust_test whose C deps are compiled /MT.
        bad: A rust_test with a C dep compiled /MD.
        target_flag: A rust_test that sets +crt-static in its own rustc_flags.
    """
    ok_name = name + "_ok"
    bad_name = name + "_bad"
    target_flag_name = name + "_target_flag"
    for test, rule, target in [(ok_name, _crt_ok_test, ok), (bad_name, _crt_bad_test, bad), (target_flag_name, _crt_target_flag_test, target_flag)]:
        rule(
            name = test,
            size = "small",
            target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
    native.test_suite(name = name, tests = [ok_name, bad_name, target_flag_name])

def rust_msvc_crt_library_tests(name, shared, static):
    """Asserts +crt-static Rust shared and static libraries reject /MD C deps on MSVC.

    Args:
        name: Name of the test suite.
        shared: A rust_shared_library with a C dep compiled /MD.
        static: A rust_static_library with a C dep compiled /MD.
    """
    tests = []
    for kind, target in [("shared", shared), ("static", static)]:
        test = "{}_{}_bad".format(name, kind)
        _crt_bad_test(
            name = test,
            size = "small",
            target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
            target_under_test = target,
        )
        tests.append(test)
    native.test_suite(name = name, tests = tests)
