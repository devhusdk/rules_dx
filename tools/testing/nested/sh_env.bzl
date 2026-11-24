"""Hands the sh toolchain's shell to the nested harness as a rustc env file."""

_SH_TOOLCHAIN = "@rules_shell//shell:toolchain_type"

def _sh_rustc_env_impl(ctx):
    toolchain = ctx.toolchains[_SH_TOOLCHAIN]
    out = ctx.actions.declare_file(ctx.label.name + ".env")
    ctx.actions.write(out, "DX_NESTED_SH_TOOLCHAIN_PATH={}\n".format(toolchain.path if toolchain else ""))
    return [DefaultInfo(files = depset([out]))]

sh_rustc_env = rule(
    implementation = _sh_rustc_env_impl,
    doc = "Writes DX_NESTED_SH_TOOLCHAIN_PATH, the target platform's sh toolchain path or empty, for rustc_env_files.",
    toolchains = [config_common.toolchain_type(_SH_TOOLCHAIN, mandatory = False)],
)
