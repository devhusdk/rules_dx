"""reStructuredText source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load("//quality:sources.bzl", "QualityToolsInfo")

def _rst_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("rst", ["rst", "rest"])], str(ctx.label)),
        # Vale converts reStructuredText through rst2html, which only the owner loads.
        QualityToolsInfo(tools = {"rst2html": ctx.attr._rst2html[DefaultInfo].files_to_run}),
    ]

rst_library = rule(
    implementation = _rst_library_impl,
    doc = "Owns the reStructuredText files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".rst", ".rest"], doc = "reStructuredText files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "reStructuredText libraries this package depends on."),
        "_rst2html": attr.label(
            default = Label("//quality/tools/python:rst2html"),
            cfg = "exec",
            executable = True,
        ),
    },
    provides = [DefaultInfo, QualityToolsInfo],
)
