"""Stand-in Ruby tool that fails until a bundle is declared."""

def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(
        out,
        "#!/bin/sh\necho 'Ruby tools need a bundle: set --@rules_dx//quality/tools/ruby:" + ctx.attr.tool + "=@<bundle>//bin:" + ctx.attr.tool + "' >&2\nexit 2\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out, files = depset([out]))]

unconfigured_tool = rule(
    implementation = _impl,
    attrs = {"tool": attr.string(mandatory = True, doc = "Tool name.")},
    executable = True,
    doc = "Executable that reports the missing bundle.",
)
