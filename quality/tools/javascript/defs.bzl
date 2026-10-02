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
setlocal ENABLEEXTENSIONS ENABLEDELAYEDEXPANSION
set "SCRIPT={script}"
set "M=%~f0.runfiles_manifest"
if exist "%RUNFILES_MANIFEST_FILE%" set "M=%RUNFILES_MANIFEST_FILE%"
if exist "%RUNFILES_DIR%\MANIFEST" set "M=%RUNFILES_DIR%\MANIFEST"
if exist "%~dp0{tree}\MANIFEST" set "M=%~dp0{tree}\MANIFEST"
set "MF=%M:/=\%"
if not exist "%MF%" echo>&2 ERROR: no runfiles manifest for {script} & exit /b 1
set "FOUND="
for /F "tokens=2* usebackq" %%i in (`%SYSTEMROOT%\system32\findstr.exe /b /l /c:"!SCRIPT! " "%MF%"`) do set "FOUND=%%i"
if not defined FOUND echo>&2 ERROR: !SCRIPT! not found in runfiles manifest & exit /b 1
set "RUNFILES=%~dp0{tree}"
set "RUNFILES_DIR=%~dp0{tree}"
set "PATH=C:\Program Files\Git\usr\bin;C:\Program Files\Git\bin;%PATH%"
sh "%FOUND:\=/%" %*
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
    body = WINDOWS_LAUNCHER.replace("{script}", key).replace("{tree}", tree)
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
