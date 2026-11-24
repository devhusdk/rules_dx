"""Windows launcher for js_binary build tools."""

visibility(["//astro/...", "//javascript/...", "//typescript/..."])

_LAUNCHER = "\r\n".join([
    "@echo off",
    "if defined RUNFILES_MANIFEST_FILE goto run",
    "set \"RUNFILES_MANIFEST_FILE=%~f0.runfiles_manifest\"",
    "if exist \"%~f0.runfiles\\MANIFEST\" set \"RUNFILES_MANIFEST_FILE=%~f0.runfiles\\MANIFEST\"",
    ":run",
    "call \"%~dp0{tool}\" %*",
    "exit /b %ERRORLEVEL%",
    "",
])

def _bin_relative(ctx, file):
    prefix = ctx.bin_dir.path + "/"
    if not file.path.startswith(prefix):
        fail("{} is not in the output tree".format(file.path))
    return file.path[len(prefix):]

def _windows_js_tool_launcher_impl(ctx):
    executable = ctx.executable.tool
    launcher = ctx.actions.declare_file(ctx.label.name + ".bat")
    rel = "../" * _bin_relative(ctx, launcher).count("/") + _bin_relative(ctx, executable)
    ctx.actions.write(
        output = launcher,
        content = _LAUNCHER.format(tool = rel.replace("/", "\\")),
        is_executable = True,
    )
    runfiles = ctx.attr.tool[DefaultInfo].default_runfiles.merge(ctx.runfiles(files = [executable]))
    return [
        DefaultInfo(executable = launcher, runfiles = runfiles),
        OutputGroupInfo(execroot_data_files = ctx.attr.tool[OutputGroupInfo].execroot_data_files),
    ]

windows_js_tool_launcher = rule(
    implementation = _windows_js_tool_launcher_impl,
    doc = "Runs a js_binary tool on Windows with RUNFILES_MANIFEST_FILE set to the launcher's runfiles.",
    attrs = {
        "tool": attr.label(
            doc = "The js_binary to run.",
            executable = True,
            cfg = "target",
            mandatory = True,
        ),
    },
    executable = True,
)

def js_build_tool(name, tool, tags = None, testonly = False, visibility = None):
    """Defines `name` as `tool`, wrapped in `windows_js_tool_launcher` on Windows; the launcher also gets `manual`."""
    launcher_tags = list(tags or [])
    if "manual" not in launcher_tags:
        launcher_tags.append("manual")
    windows_js_tool_launcher(
        name = name + "_windows",
        tool = tool,
        tags = launcher_tags,
        testonly = testonly,
        visibility = visibility,
    )
    native.alias(
        name = name,
        actual = select({
            Label("@platforms//os:windows"): ":" + name + "_windows",
            "//conditions:default": tool,
        }),
        tags = tags,
        testonly = testonly,
        visibility = visibility,
    )
