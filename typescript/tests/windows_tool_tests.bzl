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
    asserts.true(env, hasattr(target[OutputGroupInfo], "execroot_data_files"), "shim lacks the execroot_data_files output group")
    return analysistest.end(env)

shim_test = analysistest.make(
    _shim_impl,
    attrs = {"tool": attr.label(default = "@npm_typescript//:tsc", executable = True, cfg = "target")},
)

def _windows_select_impl(ctx):
    env = analysistest.begin(ctx)
    executable = analysistest.target_under_test(env)[DefaultInfo].files_to_run.executable
    asserts.equals(env, ctx.attr.basename, executable.basename)
    return analysistest.end(env)

windows_select_test = analysistest.make(
    _windows_select_impl,
    attrs = {"basename": attr.string(mandatory = True)},
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

_CASES = [
    ("tsc", "//typescript/private:tsc", "@npm_typescript//:tsc"),
    ("vite", "//javascript/private:vite_build_tool", "//javascript/private:vite_tool"),
    ("svelte_tsc", "//javascript/private/quality/bin:svelte_tsc_tool", "//javascript/private/quality/bin:svelte_tsc"),
    ("protoc_gen_es", "//javascript/private/quality/bin:protoc_gen_es_tool", "//javascript/private/quality/bin:protoc_gen_es"),
]

def windows_tool_tests(name):
    """Declares the shim and select tests for each js_build_tool case.

    Args:
        name: Name of the test suite.
    """
    tests = []
    for case, alias, tool in _CASES:
        prefix = "{}_{}".format(name, case)
        shim_test(name = prefix + "_shim", target_under_test = alias + "_windows", tool = tool, size = "small")
        windows_select_test(
            name = prefix + "_windows_select",
            target_under_test = alias,
            basename = Label(alias).name + "_windows.bat",
            size = "small",
        )
        default_select_test(name = prefix + "_default_select", target_under_test = alias, tool = tool, size = "small")
        tests += [prefix + "_shim", prefix + "_windows_select", prefix + "_default_select"]
    native.test_suite(name = name, tests = tests)
