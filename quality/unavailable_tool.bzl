"""Marks an executable tool that has no build for the current execution platform."""

DxUnavailableToolInfo = provider(
    doc = "Marks a stub standing in for a tool that is unavailable on this execution platform.",
    fields = {"tool": "The tool name."},
)

def _dx_unavailable_tool_impl(ctx):
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        script,
        "#!/bin/sh\necho '" + ctx.attr.tool + " is unavailable on this execution platform' >&2\nexit 2\n",
        is_executable = True,
    )
    return [
        DefaultInfo(executable = script),
        DxUnavailableToolInfo(tool = ctx.attr.tool),
    ]

dx_unavailable_tool = rule(
    implementation = _dx_unavailable_tool_impl,
    attrs = {"tool": attr.string(doc = "Name of the tool that is unavailable.", mandatory = True)},
    executable = True,
    doc = "An executable that refuses to run, selected where a tool has no build.",
)
