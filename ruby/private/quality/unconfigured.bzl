"""Stand-in Ruby tool that fails until a bundle is declared."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")

def _impl(ctx):
    launcher, _, runfiles = write_native_launcher(ctx, "fail=Ruby tools need a bundle: set --@rules_dx//ruby/private/quality:" + ctx.attr.tool + "=@<bundle>//bin:" + ctx.attr.tool + "\n")
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

unconfigured_tool = rule(
    implementation = _impl,
    attrs = NATIVE_LAUNCHER_ATTRS | {"tool": attr.string(mandatory = True, doc = "Tool name.")},
    executable = True,
    doc = "Executable that reports the missing bundle.",
)
