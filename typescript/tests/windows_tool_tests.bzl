"""Analysis tests for the Windows tsc shim."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _bin_relative(ctx, file):
    return file.path[len(ctx.bin_dir.path) + 1:]

def _shim_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    shim = target[DefaultInfo].files_to_run.executable
    tool = ctx.executable.tool
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite" and shim in a.outputs.to_list()]
    asserts.equals(env, 1, len(writes))
    content = writes[0].content
    for line in [
        "if defined RUNFILES_MANIFEST_FILE goto run\r\n",
        "set \"RUNFILES_MANIFEST_FILE=%~f0.runfiles_manifest\"\r\n",
        "if exist \"%~f0.runfiles\\MANIFEST\" set \"RUNFILES_MANIFEST_FILE=%~f0.runfiles\\MANIFEST\"\r\n",
    ]:
        asserts.true(env, line in content, "missing line: " + line)
    rel = "../" * _bin_relative(ctx, shim).count("/") + _bin_relative(ctx, tool)
    call = "call \"%~dp0" + rel.replace("/", "\\") + "\" %*\r\n"
    asserts.true(env, call in content, "expected %r in %r" % (call, content))
    asserts.true(env, tool in target[DefaultInfo].default_runfiles.files.to_list(), "shim runfiles lack " + tool.path)
    return analysistest.end(env)

shim_test = analysistest.make(
    _shim_impl,
    attrs = {"tool": attr.label(default = "@npm_typescript//:tsc", executable = True, cfg = "target")},
)

def _windows_select_impl(ctx):
    env = analysistest.begin(ctx)
    executable = analysistest.target_under_test(env)[DefaultInfo].files_to_run.executable
    asserts.equals(env, "tsc_windows.bat", executable.basename)
    return analysistest.end(env)

windows_select_test = analysistest.make(
    _windows_select_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//tools/platforms:windows_x86_64"))},
)

def _default_select_impl(ctx):
    env = analysistest.begin(ctx)
    executable = analysistest.target_under_test(env)[DefaultInfo].files_to_run.executable
    asserts.equals(env, ctx.executable.tool, executable)
    return analysistest.end(env)

default_select_test = analysistest.make(
    _default_select_impl,
    attrs = {"tool": attr.label(default = "@npm_typescript//:tsc", executable = True, cfg = "target")},
)

def windows_tool_tests(name):
    shim_test(name = name + "_shim", target_under_test = "//typescript/private:tsc_windows", size = "small")
    windows_select_test(name = name + "_windows_select", target_under_test = "//typescript/private:tsc", size = "small")
    default_select_test(name = name + "_default_select", target_under_test = "//typescript/private:tsc", size = "small")
    native.test_suite(name = name, tests = [name + "_shim", name + "_windows_select", name + "_default_select"])
