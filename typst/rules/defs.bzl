"""Typst source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _typst_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("typst", ["typ"])], str(ctx.label)),
    ]

typst_library = rule(
    implementation = _typst_library_impl,
    doc = "Owns the Typst files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".typ"], doc = "Typst files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Typst libraries this package depends on."),
    },
)
