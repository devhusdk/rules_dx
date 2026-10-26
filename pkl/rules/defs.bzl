"""Pkl source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _pkl_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("pkl", ["pkl"])], str(ctx.label)),
    ]

pkl_library = rule(
    implementation = _pkl_library_impl,
    doc = "Owns the Pkl files of one package for dx format.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".pkl"], doc = "Pkl files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Pkl libraries this package imports."),
    },
)
