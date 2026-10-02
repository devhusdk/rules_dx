r"""Platform-correct launchers for the generated JavaScript tool binaries.

`rules_nodejs` writes its Windows launcher as a batch file that hands the tool's script to
`C:/WINDOWS/system32/bash.exe`. That is WSL's bash, which cannot read a `C:/...` path, so
every runfile lookup inside the script fails on a Windows host. Git's `sh` reads those paths
and ships with every Windows image that has a toolchain, and this repository already relies
on it for `starlark_test`.

The generated script is what runs on every host. Only the wrapper differs, and only where the
generated launcher cannot be used at all.
"""

WINDOWS_LAUNCHER = r"""@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "TREE=%~dp0{tree}"
set "SCRIPT=%TREE%\{script}"
if exist "%SCRIPT%" goto :run
set "M=%~f0.runfiles_manifest"
if not exist "%M%" set "M=%TREE%\MANIFEST"
set "MF=%M:/=\%"
if exist "%MF%" for /F "tokens=2* usebackq" %%i in (`%SYSTEMROOT%\system32\findstr.exe /b /l /c:"{key} " "%MF%"`) do if not defined FOUND set "SCRIPT=%%i"
if not exist "%SCRIPT%" (
  echo>&2 ERROR: {key} not found beside this launcher
  exit /b 1
)
:run
set "RUNFILES=%TREE%"
set "RUNFILES_DIR=%TREE%"
set "PATH=C:\Program Files\Git\usr\bin;C:\Program Files\Git\bin;%PATH%"
rem Git's sh reads a drive-relative path, so spell this one the way MSYS spells it.
set "SH_SCRIPT=!SCRIPT:\=/!"
set "SH_SCRIPT=/!SH_SCRIPT:~0,1!!SH_SCRIPT:~2!"
sh "!SH_SCRIPT!" %*
exit /b %ERRORLEVEL%
"""

def _windows_os(ctx):
    """Returns whether this tool runs on a Windows host."""
    return ctx.target_platform_has_constraint(
        ctx.attr._windows_os[platform_common.ConstraintValueInfo],
    )

def _generated_script(executable):
    """Returns the script the generated launcher runs.

    `rules_nodejs` keeps the shell script beside the batch file under the same name, so the
    script is the executable without its suffix, and both are in the runfiles. Where there is
    no suffix the executable already is the script.
    """
    short_path = executable.short_path
    for suffix in (".bat", ".exe", ".cmd"):
        if short_path.endswith(suffix):
            return short_path[:-len(suffix)]
    return short_path

def _js_tool_binary_impl(ctx):
    target = ctx.attr.js_binary[DefaultInfo]
    generated = target.files_to_run
    if not _windows_os(ctx):
        # A link keeps the generated script's own runfiles resolution unchanged.
        script = _generated_script(generated.executable)
        link = ctx.actions.declare_symlink(ctx.label.name + ".sh")
        ctx.actions.symlink(
            output = link,
            target_path = script.split(ctx.label.package + "/", 1)[-1],
        )
        return [DefaultInfo(
            executable = link,
            files = depset([link, generated.executable]),
            runfiles = target.default_runfiles,
        )]

    script = _generated_script(generated.executable)
    key = (ctx.workspace_name + "/" + script).replace("\\", "/")
    tree = ctx.label.name + ".bat.runfiles"
    launcher = ctx.actions.declare_file(ctx.label.name + ".bat")
    body = (WINDOWS_LAUNCHER
        .replace("{tree}", tree)
        .replace("{script}", key.replace("/", "\\"))
        .replace("{key}", key))
    ctx.actions.write(launcher, body, is_executable = True)
    return [DefaultInfo(
        executable = launcher,
        runfiles = target.default_runfiles.merge(ctx.runfiles(files = [launcher])),
    )]

js_tool_binary = rule(
    doc = "Runs a generated JavaScript tool binary on every host.",
    implementation = _js_tool_binary_impl,
    attrs = {
        "js_binary": attr.label(
            mandatory = True,
            cfg = "exec",
            allow_files = True,
        ),
        "_windows_os": attr.label(default = "@platforms//os:windows"),
    },
    executable = True,
)

def js_tool(name, js_binary, **kwargs):
    """Declares one platform-correct wrapper around a generated JavaScript tool."""
    js_tool_binary(
        name = name,
        js_binary = js_binary,
        **kwargs
    )
