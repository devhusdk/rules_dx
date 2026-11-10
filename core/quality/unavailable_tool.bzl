"""Marks an executable tool that has no build for the current execution platform."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")

DxUnavailableToolInfo = provider(
    doc = "Marks a stub standing in for a tool that is unavailable on this execution platform.",
    fields = {"tool": "The tool name."},
)

def _dx_unavailable_tool_impl(ctx):
    launcher, _, runfiles = write_native_launcher(ctx, "fail=" + ctx.attr.tool + " is unavailable on this execution platform\n")
    return [
        DefaultInfo(executable = launcher, runfiles = runfiles),
        DxUnavailableToolInfo(tool = ctx.attr.tool),
    ]

dx_unavailable_tool = rule(
    implementation = _dx_unavailable_tool_impl,
    attrs = NATIVE_LAUNCHER_ATTRS | {"tool": attr.string(doc = "Name of the tool that is unavailable.", mandatory = True)},
    executable = True,
    doc = "An executable that refuses to run, selected where a tool has no build.",
)
