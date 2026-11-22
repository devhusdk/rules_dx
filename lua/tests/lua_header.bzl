"""Exposes one public source header of a cc library as a file target."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

def _lua_header_impl(ctx):
    headers = [
        header
        for header in ctx.attr.lib[CcInfo].compilation_context.direct_public_headers
        if header.is_source and header.basename == ctx.attr.header
    ]
    if len(headers) != 1:
        fail("{} has {} public source headers named {}".format(ctx.attr.lib.label, len(headers), ctx.attr.header))
    return [DefaultInfo(files = depset(headers))]

lua_header = rule(
    implementation = _lua_header_impl,
    attrs = {
        "header": attr.string(mandatory = True),
        "lib": attr.label(mandatory = True, providers = [CcInfo]),
    },
)
