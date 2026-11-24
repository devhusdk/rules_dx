"""Fails analysis unless the target platform resolves the source-built Julia toolchain."""

_JULIA_TOOLCHAIN = "@rules_julia//julia:toolchain_type"

def _impl(ctx):
    julia = ctx.toolchains[_JULIA_TOOLCHAIN].julia
    if "/source_julia/" not in julia.path:
        fail("{} resolved the Julia toolchain at {}, want the source_julia tree".format(ctx.label, julia.path))
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, julia.path + "\n")
    return DefaultInfo(files = depset([out]))

resolved_source_julia_toolchain = rule(
    implementation = _impl,
    toolchains = [_JULIA_TOOLCHAIN],
)
