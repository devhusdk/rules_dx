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
    asserts.equals(env, ctx.attr.tool.label, executable.owner)
    return analysistest.end(env)

default_select_test = analysistest.make(
    _default_select_impl,
    attrs = {"tool": attr.label(default = "@npm_typescript//:tsc", executable = True, cfg = "target")},
    config_settings = {"//command_line_option:platforms": str(Label("//tools/platforms:linux_x86_64"))},
)

_UPSTREAM_CHANGED = "the upstream Windows js_binary launcher changed; check whether js_build_tool can be removed (#13889)"
_RLOCATION_START = ":: Start of rlocation"
_RLOCATION_END = ":: End of rlocation"

def _upstream_launcher_impl(ctx):
    env = analysistest.begin(ctx)
    executable = analysistest.target_under_test(env)[DefaultInfo].files_to_run.executable
    asserts.equals(env, "tsc.bat", executable.basename, _UPSTREAM_CHANGED)
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite" and executable in a.outputs.to_list()]
    asserts.equals(env, 1, len(writes), _UPSTREAM_CHANGED)
    if len(writes) != 1:
        return analysistest.end(env)
    content = writes[0].content
    if _RLOCATION_START not in content or _RLOCATION_END not in content:
        asserts.true(env, False, _UPSTREAM_CHANGED)
        return analysistest.end(env)
    body = content[:content.index(_RLOCATION_START)] + content[content.index(_RLOCATION_END):]
    asserts.true(env, "if not exist \"!run_script!\" call :rlocation" in body, _UPSTREAM_CHANGED)
    asserts.false(env, "RUNFILES_MANIFEST_FILE" in body, _UPSTREAM_CHANGED)
    return analysistest.end(env)

upstream_launcher_test = analysistest.make(
    _upstream_launcher_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//tools/platforms:windows_x86_64"))},
)

_CASES = [
    ("tsc", "//typescript/private:tsc", "@npm_typescript//:tsc"),
    ("vite", "//javascript/private:vite_build_tool", "//javascript/private:vite_tool"),
    ("svelte_tsc", "//javascript/private/quality/bin:svelte_tsc_tool", "//javascript/private/quality/bin:svelte_tsc"),
    ("protoc_gen_es", "//javascript/private/quality/bin:protoc_gen_es_tool", "//javascript/private/quality/bin:protoc_gen_es"),
    ("typedoc", "//javascript/private/quality/bin:typedoc_tool", "//javascript/private/quality/bin:typedoc"),
    ("svelte_component_meta", "//javascript/private/quality/bin:svelte_component_meta_tool", "//javascript/private/quality/bin:svelte_component_meta"),
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
    upstream_launcher_test(name = name + "_upstream_launcher", target_under_test = "@npm_typescript//:tsc", size = "small")
    tests.append(name + "_upstream_launcher")
    native.test_suite(name = name, tests = tests)

def ts_project_tool_tests(name, project):
    """Declares the shim and select tests for the per-project tsc and validator tools.

    Args:
        name: Name of the test suite.
        project: Name of the ts_project in the calling package.
    """
    tests = []
    for tool in ["tsc", "validator"]:
        prefix = "{}_{}".format(name, tool)
        alias = ":{}_{}".format(project, tool)
        upstream = Label("@npm_typescript//:" + tool)
        shim_test(name = prefix + "_shim", target_under_test = alias + "_windows", tool = upstream, size = "small")
        windows_select_test(
            name = prefix + "_windows_select",
            target_under_test = alias,
            basename = "{}_{}_windows.bat".format(project, tool),
            size = "small",
        )
        default_select_test(name = prefix + "_default_select", target_under_test = alias, tool = upstream, size = "small")
        tests += [prefix + "_shim", prefix + "_windows_select", prefix + "_default_select"]
    native.test_suite(name = name, tests = tests)
