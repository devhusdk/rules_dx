"""MSIX tool toolchain."""

def _msix_toolchain_impl(ctx):
    tools = [f for f in [ctx.file.makeappx, ctx.file.signtool] if f]
    return [DefaultInfo(files = depset(tools)), platform_common.ToolchainInfo(
        error = ctx.attr.error,
        files = depset(ctx.files.files),
        makeappx = ctx.file.makeappx,
        signtool = ctx.file.signtool,
        version = ctx.attr.version,
    )]

msix_toolchain = rule(
    implementation = _msix_toolchain_impl,
    doc = "Declares makeappx and signtool for one Windows execution arch.",
    attrs = {
        "error": attr.string(doc = "Why the tools are unavailable. Empty when they are usable."),
        "files": attr.label_list(doc = "Every file the tools load.", allow_files = True, cfg = "exec"),
        "makeappx": attr.label(doc = "makeappx.exe.", allow_single_file = True, cfg = "exec"),
        "signtool": attr.label(doc = "signtool.exe.", allow_single_file = True, cfg = "exec"),
        "version": attr.string(doc = "Windows SDK Build Tools version."),
    },
)

def _signtool_impl(ctx):
    toolchain = ctx.toolchains[str(Label("//msix:toolchain_type"))]
    if toolchain == None:
        fail("dx_msix_sign: signing needs a windows_x86_64 or windows_arm64 execution platform, and this build has none")
    if toolchain.error:
        fail(toolchain.error)
    return [DefaultInfo(
        files = depset([toolchain.signtool]),
        runfiles = ctx.runfiles(transitive_files = toolchain.files),
    )]

msix_signtool = rule(
    implementation = _signtool_impl,
    doc = "Provides the signtool that runs on the execution platform.",
    toolchains = [config_common.toolchain_type("//msix:toolchain_type", mandatory = False)],
)
