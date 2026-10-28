"""Rego source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "RegoInfo")

def _rego_library_impl(ctx):
    transitive_srcs = depset(
        ctx.files.srcs,
        transitive = [dep[RegoInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    transitive_data = depset(
        ctx.files.data,
        transitive = [dep[RegoInfo].transitive_data for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(transitive = [transitive_srcs, transitive_data])),
        RegoInfo(transitive_srcs = transitive_srcs, transitive_data = transitive_data),
        dx_quality_sources(ctx.files.srcs, [("rego", ["rego"])], str(ctx.label)),
    ]

rego_library = rule(
    implementation = _rego_library_impl,
    doc = "Owns the Rego files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".rego"], doc = "Rego files of this package."),
        "data": attr.label_list(allow_files = [".json", ".yaml", ".yml"], doc = "Data files loaded with the Rego files; not checked by dx quality."),
        "deps": attr.label_list(providers = [RegoInfo], doc = "Rego libraries this package depends on."),
    },
)
