"""Analysis tests for the Windows py_binary launcher and venv."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_WINDOWS = str(Label("//msix/platforms:windows_x86_64"))
_LINUX = str(Label("//appimage/platforms:linux_x86_64"))

def _launcher_argv(env):
    """Returns the embedded launcher args, the part of the stub action argv after `--`."""
    argv = [a for a in analysistest.target_actions(env) if a.mnemonic == "Action"][0].argv
    return argv[argv.index("--") + 1:]

def _windows_launcher_impl(ctx):
    env = analysistest.begin(ctx)
    args = _launcher_argv(env)
    asserts.true(env, args[0].endswith("/python.exe"), "launcher entrypoint " + args[0])
    asserts.false(env, "/bin/" in args[0], "launcher runs the toolchain python, not the venv link: " + args[0])
    asserts.true(env, "windows" in args[0], "launcher entrypoint is the windows toolchain: " + args[0])
    asserts.true(env, len(args) >= 3, "args " + str(args))
    asserts.true(env, args[-2].endswith("/bin/run.py"), "run.py comes right before main: " + str(args))
    asserts.true(env, args[-1].endswith("/main.py"), "main comes last: " + str(args))
    return analysistest.end(env)

def _linux_launcher_impl(ctx):
    env = analysistest.begin(ctx)
    args = _launcher_argv(env)
    asserts.true(env, args[0].endswith("/bin/python"), "launcher entrypoint " + args[0])
    asserts.true(env, args[-1].endswith("/main.py"), "main comes last: " + str(args))
    asserts.equals(env, [], [a for a in args if a.endswith("run.py")])
    return analysistest.end(env)

def _outputs_by_mnemonic(env):
    """Maps each venv output basename under bin/ to the mnemonic of the action that makes it."""
    found = {}
    for action in analysistest.target_actions(env):
        for f in action.outputs.to_list():
            if f.dirname.endswith("/bin"):
                found[f.basename] = action.mnemonic
    return found

def _windows_venv_impl(ctx):
    env = analysistest.begin(ctx)
    found = _outputs_by_mnemonic(env)
    asserts.true(env, found.get("python.exe") in ["Symlink", "ExecutableSymlink"], "bin/python.exe action " + str(found))
    asserts.equals(env, "FileWrite", found.get("run.py"))
    asserts.equals(env, [], [n for n, m in found.items() if m == "UnresolvedSymlink"])
    return analysistest.end(env)

def _linux_venv_impl(ctx):
    env = analysistest.begin(ctx)
    found = _outputs_by_mnemonic(env)
    asserts.equals(env, "UnresolvedSymlink", found.get("python"))
    asserts.equals(env, None, found.get("run.py"))
    asserts.equals(env, None, found.get("python.exe"))
    return analysistest.end(env)

def _make(impl, platform):
    return analysistest.make(impl, config_settings = {"//command_line_option:platforms": platform})

windows_launcher_test = _make(_windows_launcher_impl, _WINDOWS)
linux_launcher_test = _make(_linux_launcher_impl, _LINUX)
windows_venv_test = _make(_windows_venv_impl, _WINDOWS)
linux_venv_test = _make(_linux_venv_impl, _LINUX)

def python_windows_launcher_tests(name, binary = "hello"):
    """Instantiates the Windows and Linux launcher analysis tests for a python_binary in this package."""
    launcher = ":{}_upstream".format(binary)
    venv = ":_{}_upstream.venv".format(binary)
    windows_launcher_test(name = name + "_windows_launcher", target_under_test = launcher, testonly = True)
    linux_launcher_test(name = name + "_linux_launcher", target_under_test = launcher, testonly = True)
    windows_venv_test(name = name + "_windows_venv", target_under_test = venv, testonly = True)
    linux_venv_test(name = name + "_linux_venv", target_under_test = venv, testonly = True)
    native.test_suite(
        name = name,
        tests = [
            name + "_windows_launcher",
            name + "_linux_launcher",
            name + "_windows_venv",
            name + "_linux_venv",
        ],
    )
