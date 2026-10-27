"""The Lua toolchain: one launcher for the target and luac for the exec platform."""

LUA_TOOLCHAIN_TYPE = Label("//lua/toolchain:toolchain_type")

def _lua_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(
        launcher = ctx.executable.launcher,
        launcher_runfiles = ctx.attr.launcher[DefaultInfo].default_runfiles,
        luac = ctx.attr.luac[DefaultInfo].files_to_run,
        version = ctx.attr.version,
    )]

lua_toolchain = rule(
    implementation = _lua_toolchain_impl,
    doc = "Declares the Lua launcher, the exec luac and the Lua version of one toolchain.",
    attrs = {
        "launcher": attr.label(
            mandatory = True,
            executable = True,
            cfg = "target",
            doc = "Launcher binary that embeds Lua and runs one entry file; built for the target platform.",
        ),
        "luac": attr.label(
            mandatory = True,
            executable = True,
            cfg = "exec",
            doc = "luac compiler that checks Lua sources at build time; built for the exec platform.",
        ),
        "version": attr.string(
            mandatory = True,
            doc = "Version of the lua Bazel module the launcher and luac are built from.",
        ),
    },
)
