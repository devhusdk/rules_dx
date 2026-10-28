"""Jupyter notebook source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _jupyter_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("jupyter", ["ipynb"])], str(ctx.label)),
    ]

jupyter_library = rule(
    implementation = _jupyter_library_impl,
    doc = "Owns the Jupyter notebooks of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".ipynb"], doc = "Python Jupyter notebooks (nbformat 4) to format and lint."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Notebook libraries these notebooks depend on."),
    },
)
