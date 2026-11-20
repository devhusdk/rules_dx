"""Windows launcher shim for js_binary build tools. Removal tracked in #13889 (bazel-lib#1296)."""

visibility(["//astro/...", "//javascript/...", "//typescript/..."])

_SHIM = "\r\n".join([
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

def _windows_js_tool_shim_impl(ctx):
    executable = ctx.executable.tool
    shim = ctx.actions.declare_file(ctx.label.name + ".bat")
    rel = "../" * _bin_relative(ctx, shim).count("/") + _bin_relative(ctx, executable)
    ctx.actions.write(
        output = shim,
        content = _SHIM.format(tool = rel.replace("/", "\\")),
        is_executable = True,
    )
    runfiles = ctx.attr.tool[DefaultInfo].default_runfiles.merge(ctx.runfiles(files = [executable]))
    return [
        DefaultInfo(executable = shim, runfiles = runfiles),
        OutputGroupInfo(execroot_data_files = ctx.attr.tool[OutputGroupInfo].execroot_data_files),
    ]

windows_js_tool_shim = rule(
    implementation = _windows_js_tool_shim_impl,
    doc = "Runs a js_binary tool on Windows with RUNFILES_MANIFEST_FILE set to the shim's runfiles.",
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

def js_build_tool(name, tool, tags = None, visibility = None):
    """Defines `name` as `tool`, wrapped in `windows_js_tool_shim` on Windows.

    Args:
        name: Name of the alias.
        tool: The js_binary to run.
        tags: Tags of the alias; the shim also gets `manual`.
        visibility: Visibility of the alias.
    """
    shim_tags = list(tags or [])
    if "manual" not in shim_tags:
        shim_tags.append("manual")
    windows_js_tool_shim(
        name = name + "_windows",
        tool = tool,
        tags = shim_tags,
        visibility = visibility,
    )
    native.alias(
        name = name,
        actual = select({
            Label("@platforms//os:windows"): ":" + name + "_windows",
            "//conditions:default": tool,
        }),
        tags = tags,
        visibility = visibility,
    )
