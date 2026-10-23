"""MSIX tool toolchain."""

def _msix_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(
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

def _unavailable_impl(ctx):
    fail(ctx.attr.error)

msix_unavailable = rule(
    implementation = _unavailable_impl,
    doc = "Fails analysis with the reason the MSIX tools are unavailable.",
    attrs = {
        "error": attr.string(doc = "Why the tools are unavailable.", mandatory = True),
    },
)
