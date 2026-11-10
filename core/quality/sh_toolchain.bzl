"""Shell toolchain backed by a pinned executable."""

def _dx_sh_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(
        path = ctx.file.executable.path,
        executable = ctx.file.executable,
    )]

dx_sh_toolchain = rule(
    implementation = _dx_sh_toolchain_impl,
    attrs = {
        "executable": attr.label(allow_single_file = True, mandatory = True),
    },
    doc = "A bazel_tools sh toolchain whose shell is a declared input.",
)
