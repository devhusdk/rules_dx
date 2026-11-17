"""Forwards an executable built for the execution platform."""

def _exec_tool_impl(ctx):
    tool = ctx.attr.tool[DefaultInfo]
    return [DefaultInfo(
        files = depset([tool.files_to_run.executable]),
        runfiles = tool.default_runfiles,
    )]

exec_tool = rule(
    implementation = _exec_tool_impl,
    doc = "Exposes `tool` built for the execution platform.",
    attrs = {
        "tool": attr.label(doc = "Executable to build for the execution platform.", cfg = "exec", executable = True, mandatory = True),
    },
)
