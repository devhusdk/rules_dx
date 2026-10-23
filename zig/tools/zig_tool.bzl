"""Exposes the resolved Zig compiler as an executable target."""

def _zig_tool_impl(ctx):
    info = ctx.toolchains["@rules_zig//zig:toolchain_type"].zigtoolchaininfo
    zig = info.zig_files[0]
    exe = ctx.actions.declare_file(ctx.label.name + "/" + zig.basename)
    ctx.actions.symlink(output = exe, target_file = zig, is_executable = True)
    return [DefaultInfo(
        executable = exe,
        files = depset([exe]),
        runfiles = ctx.runfiles(files = info.zig_files),
    )]

zig_tool = rule(
    implementation = _zig_tool_impl,
    executable = True,
    toolchains = ["@rules_zig//zig:toolchain_type"],
    doc = "The Zig compiler from the resolved rules_zig toolchain.",
)
