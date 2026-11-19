"""Runs a js_binary as an action tool on Windows with its runfiles located."""

visibility("private")

# The bazel_lib Windows launcher starts the sibling bash script without exporting a runfiles
# location, and the bash script then looks for <name>.sh.runfiles, which Bazel never creates.
# This launcher points RUNFILES_MANIFEST_FILE at its own runfiles, which hold the tool's.
_WINDOWS_LAUNCHER = """@echo off\r
if not defined RUNFILES_DIR if not defined RUNFILES_MANIFEST_FILE (\r
  if exist "%~f0.runfiles\\MANIFEST" (\r
    set "RUNFILES_MANIFEST_FILE=%~f0.runfiles\\MANIFEST"\r
  ) else (\r
    set "RUNFILES_MANIFEST_FILE=%~f0.runfiles_manifest"\r
  )\r
)\r
call "%~dp0{tool}" %*\r
exit /b %ERRORLEVEL%\r
"""

def _bin_relative(ctx, file):
    prefix = ctx.bin_dir.path + "/"
    if not file.path.startswith(prefix):
        fail("{} is not in the output tree".format(file.path))
    return file.path[len(prefix):]

def _action_tool_impl(ctx):
    tool = ctx.attr.tool[DefaultInfo]
    executable = ctx.executable.tool
    if ctx.target_platform_has_constraint(ctx.attr._windows[platform_common.ConstraintValueInfo]):
        launcher = ctx.actions.declare_file(ctx.label.name + ".bat")
        depth = _bin_relative(ctx, launcher).count("/")
        target = "../" * depth + _bin_relative(ctx, executable)
        ctx.actions.write(
            output = launcher,
            content = _WINDOWS_LAUNCHER.format(tool = target.replace("/", "\\")),
            is_executable = True,
        )
    else:
        launcher = ctx.actions.declare_file(ctx.label.name)
        ctx.actions.symlink(output = launcher, target_file = executable, is_executable = True)
    runfiles = ctx.runfiles(files = [executable]).merge(tool.default_runfiles)
    return [DefaultInfo(executable = launcher, runfiles = runfiles)]

action_tool = rule(
    implementation = _action_tool_impl,
    doc = "Wraps a js_binary so it runs as an action tool on every execution platform.",
    attrs = {
        "tool": attr.label(
            doc = "The js_binary to run.",
            executable = True,
            cfg = "target",
            mandatory = True,
        ),
        "_windows": attr.label(default = "@platforms//os:windows"),
    },
    executable = True,
)
