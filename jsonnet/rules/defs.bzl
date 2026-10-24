"""Jsonnet source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _jsonnet_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("jsonnet", ["jsonnet", "libsonnet"])], str(ctx.label)),
    ]

jsonnet_library = rule(
    implementation = _jsonnet_library_impl,
    doc = "Owns the Jsonnet files of one package for dx format.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".jsonnet", ".libsonnet"], doc = "Jsonnet files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Jsonnet libraries this package imports."),
    },
)
