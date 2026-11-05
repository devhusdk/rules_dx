"""The xref tool: the OTP escript launcher plus the driver script it runs."""

load(":dialyzer.bzl", "tool_launcher")
load(":providers.bzl", "XrefToolInfo")

def _xref_tool_impl(ctx):
    launcher, runfiles = tool_launcher(ctx, ctx.attr.binary)
    return [
        DefaultInfo(
            executable = launcher,
            files = depset([launcher]),
            runfiles = runfiles,
        ),
        XrefToolInfo(script = ctx.file.script),
    ]

xref_tool = rule(
    implementation = _xref_tool_impl,
    doc = "The escript launcher plus the xref driver script `dx check` runs it with.",
    executable = True,
    attrs = {
        "binary": attr.label(mandatory = True, executable = True, cfg = "target", doc = "An otp_tool_binary for escript."),
        "script": attr.label(mandatory = True, allow_single_file = [".escript"], doc = "The xref driver script."),
    },
    provides = [DefaultInfo, XrefToolInfo],
)
