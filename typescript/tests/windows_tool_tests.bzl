"""Analysis tests for the Windows tsc shim."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts", "unittest")

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

_RUN_SCRIPT_RLOCATION = "if not exist \"!run_script!\" call :rlocation"
_RUNFILES_UNSET = "RUNFILES environment variable is not set"

def _upstream_fix_signals(bat_content, init_runfiles):
    """Lists the ways the upstream launcher pair differs from the broken one; empty while it is still broken.

    Args:
        bat_content: Content of bazel-lib's generated `.bat` launcher.
        init_runfiles: rules_js's `{{initialize_runfiles}}` substitution, or None if absent.

    Returns:
        A list of signal strings.
    """
    signals = []
    if _RLOCATION_START not in bat_content or _RLOCATION_END not in bat_content:
        signals.append("bat: rlocation markers missing")
    else:
        body = bat_content[:bat_content.index(_RLOCATION_START)] + bat_content[bat_content.index(_RLOCATION_END):]
        if _RUN_SCRIPT_RLOCATION not in body:
            signals.append("bat: run_script rlocation fallback missing")
        if "RUNFILES_MANIFEST_FILE" in body:
            signals.append("bat: body sets RUNFILES_MANIFEST_FILE")
    if init_runfiles == None:
        signals.append("bash: {{initialize_runfiles}} substitution missing")
    else:
        if _RUNFILES_UNSET not in init_runfiles:
            signals.append("bash: unset RUNFILES no longer fatal")
        if ".bat" in init_runfiles:
            signals.append("bash: runfiles init mentions .bat")
    return signals

def _upstream_launcher_impl(ctx):
    env = analysistest.begin(ctx)
    executable = analysistest.target_under_test(env)[DefaultInfo].files_to_run.executable
    asserts.equals(env, "tsc.bat", executable.basename, _UPSTREAM_CHANGED)
    actions = analysistest.target_actions(env)
    writes = [a for a in actions if a.mnemonic == "FileWrite" and executable in a.outputs.to_list()]
    asserts.equals(env, 1, len(writes), _UPSTREAM_CHANGED)
    bash_path = executable.path[:-len(".bat")]
    expands = [a for a in actions if a.mnemonic == "TemplateExpand" and bash_path in [f.path for f in a.outputs.to_list()]]
    asserts.equals(env, 1, len(expands), _UPSTREAM_CHANGED)
    if len(writes) != 1 or len(expands) != 1:
        return analysistest.end(env)
    signals = _upstream_fix_signals(writes[0].content, expands[0].substitutions.get("{{initialize_runfiles}}"))
    asserts.equals(env, [], signals, _UPSTREAM_CHANGED + ": " + ", ".join(signals))
    return analysistest.end(env)

upstream_launcher_test = analysistest.make(
    _upstream_launcher_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//tools/platforms:windows_x86_64"))},
)

_FIXTURE_BAT = "\r\n".join([
    "@echo off",
    "SETLOCAL ENABLEEXTENSIONS",
    "SETLOCAL ENABLEDELAYEDEXPANSION",
    "set RUNFILES_MANIFEST_ONLY=1",
    ":: Start of rlocation",
    "goto :rlocation_end",
    ":rlocation",
    "set \"runfiles_manifest_file=%RUNFILES_MANIFEST_FILE%\"",
    ":rlocation_end",
    ":: End of rlocation",
    "set \"run_script=%~dp0tsc\"",
    "if not exist \"!run_script!\" call :rlocation \"_main/tsc_/tsc\" run_script",
    "set \"run_script=!run_script:\\=/!\"",
    "for %%a in (\"C:/bash.exe\") do set \"bash_bin_dir=%%~dpa\"",
    "set PATH=%bash_bin_dir%;%PATH%",
    "set args=%*",
    "\"C:/bash.exe\" -c \"'!run_script!' !args!\"",
    "",
])

_FIXTURE_INIT = """\
if [ "${RUNFILES_MANIFEST_FILE:-}" ]; then
    RUNFILES=${RUNFILES_MANIFEST_FILE%_manifest}
elif [ ! "${RUNFILES:-}" ]; then
    log_fatal "RUNFILES environment variable is not set"
    exit 1
fi
"""

_RUN_SCRIPT_LINE = "if not exist \"!run_script!\" call :rlocation \"_main/tsc_/tsc\" run_script\r\n"

def _upstream_fix_signals_impl(ctx):
    env = unittest.begin(ctx)
    asserts.equals(env, [], _upstream_fix_signals(_FIXTURE_BAT, _FIXTURE_INIT))
    for bat, init, signal in [
        (_FIXTURE_BAT.replace(_RLOCATION_START, ""), _FIXTURE_INIT, "bat: rlocation markers missing"),
        (_FIXTURE_BAT.replace(_RUN_SCRIPT_LINE, ""), _FIXTURE_INIT, "bat: run_script rlocation fallback missing"),
        (_FIXTURE_BAT.replace("@echo off\r\n", "@echo off\r\nset \"RUNFILES_MANIFEST_FILE=%~f0.runfiles\\MANIFEST\"\r\n"), _FIXTURE_INIT, "bat: body sets RUNFILES_MANIFEST_FILE"),
        (_FIXTURE_BAT, None, "bash: {{initialize_runfiles}} substitution missing"),
        (_FIXTURE_BAT, _FIXTURE_INIT.replace(_RUNFILES_UNSET, "RUNFILES fallback"), "bash: unset RUNFILES no longer fatal"),
        (_FIXTURE_BAT, _FIXTURE_INIT.replace("fi\n", "elif [ -d \"$0.bat.runfiles\" ]; then\n    RUNFILES=\"$0.bat.runfiles\"\nfi\n"), "bash: runfiles init mentions .bat"),
    ]:
        asserts.equals(env, [signal], _upstream_fix_signals(bat, init))
    return unittest.end(env)

upstream_fix_signals_test = unittest.make(_upstream_fix_signals_impl)

_CASES = [
    ("tsc", "//typescript/private:tsc", "@npm_typescript//:tsc"),
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
    upstream_fix_signals_test(name = name + "_upstream_fix_signals", size = "small")
    tests += [name + "_upstream_launcher", name + "_upstream_fix_signals"]
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

def framework_build_tool_tests(name, build):
    """Declares the shim and select tests for the build tool of one framework build.

    Args:
        name: Name of the test suite.
        build: Name of the framework build in the calling package.
    """
    alias = ":{}_dx_tool_build".format(build)
    tool = ":{}_dx_tool".format(build)
    shim_test(name = name + "_shim", target_under_test = alias + "_windows", tool = tool, size = "small")
    windows_select_test(
        name = name + "_windows_select",
        target_under_test = alias,
        basename = "{}_dx_tool_build_windows.bat".format(build),
        size = "small",
    )
    default_select_test(name = name + "_default_select", target_under_test = alias, tool = tool, size = "small")
    native.test_suite(name = name, tests = [name + "_shim", name + "_windows_select", name + "_default_select"])
