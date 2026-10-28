"""Exposes the resolved Zig compiler as an executable target."""

def _zig_tool_impl(ctx):
    info = ctx.toolchains["@rules_zig//zig:toolchain_type"].zigtoolchaininfo
    zig = info.zig_exe.file
    exe = ctx.actions.declare_file(ctx.label.name + "/" + zig.basename)
    ctx.actions.symlink(output = exe, target_file = zig, is_executable = True)
    return [DefaultInfo(
        executable = exe,
        files = depset([exe]),
        runfiles = ctx.runfiles(files = [info.zig_exe.file, info.zig_lib.file]),
    )]

zig_tool = rule(
    implementation = _zig_tool_impl,
    executable = True,
    toolchains = ["@rules_zig//zig:toolchain_type"],
    doc = "The Zig compiler from the resolved rules_zig toolchain.",
)

def _zig_lib_marker_impl(ctx):
    info = ctx.toolchains["@rules_zig//zig:toolchain_type"].zigtoolchaininfo
    lib = info.zig_lib.file
    return [DefaultInfo(files = depset([lib]), runfiles = ctx.runfiles(files = [lib]))]

zig_lib_marker = rule(
    implementation = _zig_lib_marker_impl,
    toolchains = ["@rules_zig//zig:toolchain_type"],
    doc = "The Zig library directory of the resolved toolchain.",
)
